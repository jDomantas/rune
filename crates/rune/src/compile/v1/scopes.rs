use crate::ast::Span;
use crate::collections::HashMap;
use crate::compile::v1::Assembler;
use crate::compile::{Assembly, CompileError, CompileErrorKind, CompileResult, CompileVisitor};
use crate::runtime::Inst;
use crate::SourceId;

/// A locally declared variable, its calculated stack offset and where it was
/// declared in its source file.
#[derive(Debug, Clone, Copy)]
pub struct Var {
    /// Slot offset from the current stack frame.
    pub(crate) offset: usize,
    /// Token assocaited with the variable.
    span: Span,
    /// Variable has been taken at the given position.
    moved_at: Option<Span>,
}

impl Var {
    /// Copy the declared variable.
    pub(crate) fn copy<C>(&self, c: &mut Assembler<'_>, span: Span, comment: C)
    where
        C: AsRef<str>,
    {
        c.asm.push_with_comment(
            Inst::Copy {
                offset: self.offset,
            },
            span,
            comment,
        );
    }

    /// Move the declared variable.
    pub(crate) fn do_move<C>(&self, asm: &mut Assembly, span: Span, comment: C)
    where
        C: AsRef<str>,
    {
        asm.push_with_comment(
            Inst::Move {
                offset: self.offset,
            },
            span,
            comment,
        );
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Scope {
    /// Named variables.
    locals: HashMap<String, Var>,
    /// The number of variables.
    pub(crate) total_var_count: usize,
    /// The number of variables local to this scope.
    pub(crate) local_var_count: usize,
}

impl Scope {
    /// Construct a new locals handlers.
    fn new() -> Scope {
        Self {
            locals: HashMap::new(),
            total_var_count: 0,
            local_var_count: 0,
        }
    }

    /// Construct a new child scope.
    fn child(&self) -> Self {
        Self {
            locals: HashMap::new(),
            total_var_count: self.total_var_count,
            local_var_count: 0,
        }
    }

    /// Insert a new local, and return the old one if there's a conflict.
    fn new_var(&mut self, name: &str, span: Span) -> CompileResult<usize> {
        let offset = self.total_var_count;

        let local = Var {
            offset,
            span,
            moved_at: None,
        };

        self.total_var_count += 1;
        self.local_var_count += 1;

        if let Some(old) = self.locals.insert(name.to_owned(), local) {
            return Err(CompileError::new(
                span,
                CompileErrorKind::VariableConflict {
                    name: name.to_owned(),
                    existing_span: old.span,
                },
            ));
        }

        Ok(offset)
    }

    /// Insert a new local, and return the old one if there's a conflict.
    fn decl_var(&mut self, name: &str, span: Span) -> usize {
        let offset = self.total_var_count;

        tracing::trace!("decl {} => {}", name, offset);

        self.locals.insert(
            name.to_owned(),
            Var {
                offset,
                span,
                moved_at: None,
            },
        );

        self.total_var_count += 1;
        self.local_var_count += 1;
        offset
    }

    /// Declare an anonymous variable.
    ///
    /// This is used if cleanup is required in the middle of an expression.
    fn decl_anon(&mut self, _span: Span) -> usize {
        let offset = self.total_var_count;
        self.total_var_count += 1;
        self.local_var_count += 1;
        offset
    }

    /// Undeclare the last anonymous variable.
    pub(crate) fn undecl_anon(&mut self, span: Span, n: usize) -> CompileResult<()> {
        self.total_var_count = self
            .total_var_count
            .checked_sub(n)
            .ok_or_else(|| CompileError::msg(&span, "totals out of bounds"))?;

        self.local_var_count = self
            .local_var_count
            .checked_sub(n)
            .ok_or_else(|| CompileError::msg(&span, "locals out of bounds"))?;

        Ok(())
    }

    /// Access the variable with the given name.
    fn get(&self, name: &str, span: Span) -> CompileResult<Option<Var>> {
        if let Some(var) = self.locals.get(name) {
            if let Some(moved_at) = var.moved_at {
                return Err(CompileError::new(
                    span,
                    CompileErrorKind::VariableMoved { moved_at },
                ));
            }

            return Ok(Some(*var));
        }

        Ok(None)
    }

    /// Access the variable with the given name.
    fn take(&mut self, name: &str, span: Span) -> CompileResult<Option<&Var>> {
        if let Some(var) = self.locals.get_mut(name) {
            if let Some(moved_at) = var.moved_at {
                return Err(CompileError::new(
                    span,
                    CompileErrorKind::VariableMoved { moved_at },
                ));
            }

            var.moved_at = Some(span);
            return Ok(Some(var));
        }

        Ok(None)
    }
}

/// A guard returned from [push][Scopes::push].
///
/// This should be provided to a subsequent [pop][Scopes::pop] to allow it to be
/// sanity checked.
#[must_use]
pub(crate) struct ScopeGuard(usize);

pub(crate) struct Scopes {
    scopes: Vec<Scope>,
}

impl Scopes {
    /// Construct a new collection of scopes.
    pub(crate) fn new() -> Self {
        Self {
            scopes: vec![Scope::new()],
        }
    }

    /// Try to get the local with the given name. Returns `None` if it's
    /// missing.
    pub(crate) fn try_get_var(
        &self,
        visitor: &mut dyn CompileVisitor,
        name: &str,
        source_id: SourceId,
        span: Span,
    ) -> CompileResult<Option<Var>> {
        tracing::trace!("get var: {}", name);

        for scope in self.scopes.iter().rev() {
            if let Some(var) = scope.get(name, span)? {
                tracing::trace!("found var: {} => {:?}", name, var);
                visitor.visit_variable_use(source_id, var.span, span);
                return Ok(Some(var));
            }
        }

        Ok(None)
    }

    /// Try to take the local with the given name. Returns `None` if it's
    /// missing.
    pub(crate) fn try_take_var(
        &mut self,
        visitor: &mut dyn CompileVisitor,
        name: &str,
        source_id: SourceId,
        span: Span,
    ) -> CompileResult<Option<&Var>> {
        tracing::trace!("get var: {}", name);

        for scope in self.scopes.iter_mut().rev() {
            if let Some(var) = scope.take(name, span)? {
                tracing::trace!("found var: {} => {:?}", name, var);
                visitor.visit_variable_use(source_id, var.span, span);
                return Ok(Some(var));
            }
        }

        Ok(None)
    }

    /// Get the local with the given name.
    pub(crate) fn get_var(
        &self,
        visitor: &mut dyn CompileVisitor,
        name: &str,
        source_id: SourceId,
        span: Span,
    ) -> CompileResult<Var> {
        match self.try_get_var(visitor, name, source_id, span)? {
            Some(var) => Ok(var),
            None => Err(CompileError::new(
                span,
                CompileErrorKind::MissingLocal {
                    name: name.to_owned(),
                },
            )),
        }
    }

    /// Take the local with the given name.
    pub(crate) fn take_var(
        &mut self,
        visitor: &mut dyn CompileVisitor,
        name: &str,
        source_id: SourceId,
        span: Span,
    ) -> CompileResult<&Var> {
        match self.try_take_var(visitor, name, source_id, span)? {
            Some(var) => Ok(var),
            None => Err(CompileError::new(
                span,
                CompileErrorKind::MissingLocal {
                    name: name.to_owned(),
                },
            )),
        }
    }

    /// Construct a new variable.
    pub(crate) fn new_var(&mut self, name: &str, span: Span) -> CompileResult<usize> {
        self.last_mut(span)?.new_var(name, span)
    }

    /// Declare the given variable.
    pub(crate) fn decl_var(&mut self, name: &str, span: Span) -> CompileResult<usize> {
        Ok(self.last_mut(span)?.decl_var(name, span))
    }

    /// Declare an anonymous variable.
    pub(crate) fn decl_anon(&mut self, span: Span) -> CompileResult<usize> {
        Ok(self.last_mut(span)?.decl_anon(span))
    }

    /// Declare an anonymous variable.
    pub(crate) fn undecl_anon(&mut self, span: Span, n: usize) -> CompileResult<()> {
        self.last_mut(span)?.undecl_anon(span, n)
    }

    /// Push a scope and return an index.
    pub(crate) fn push(&mut self, scope: Scope) -> ScopeGuard {
        self.scopes.push(scope);
        ScopeGuard(self.scopes.len())
    }

    /// Pop the last scope and compare with the expected length.
    pub(crate) fn pop(&mut self, expected: ScopeGuard, span: Span) -> CompileResult<Scope> {
        let ScopeGuard(expected) = expected;

        if self.scopes.len() != expected {
            return Err(CompileError::msg(
                &span,
                "the number of scopes do not match",
            ));
        }

        self.pop_unchecked(span)
    }

    /// Pop the last of the scope.
    pub(crate) fn pop_last(&mut self, span: Span) -> CompileResult<Scope> {
        self.pop(ScopeGuard(1), span)
    }

    /// Pop the last scope and compare with the expected length.
    pub(crate) fn pop_unchecked(&mut self, span: Span) -> CompileResult<Scope> {
        let scope = self
            .scopes
            .pop()
            .ok_or_else(|| CompileError::msg(&span, "missing parent scope"))?;

        Ok(scope)
    }

    /// Construct a new child scope and return its guard.
    pub(crate) fn push_child(&mut self, span: Span) -> CompileResult<ScopeGuard> {
        let scope = self.last(span)?.child();
        Ok(self.push(scope))
    }

    /// Construct a new child scope.
    pub(crate) fn child(&mut self, span: Span) -> CompileResult<Scope> {
        Ok(self.last(span)?.child())
    }

    /// Get the local var count of the top scope.
    pub(crate) fn local_var_count(&self, span: Span) -> CompileResult<usize> {
        Ok(self.last(span)?.local_var_count)
    }

    /// Get the total var count of the top scope.
    pub(crate) fn total_var_count(&self, span: Span) -> CompileResult<usize> {
        Ok(self.last(span)?.total_var_count)
    }

    /// Get the local with the given name.
    fn last(&self, span: Span) -> CompileResult<&Scope> {
        self.scopes
            .last()
            .ok_or_else(|| CompileError::msg(&span, "missing head of locals"))
    }

    /// Get the last locals scope.
    fn last_mut(&mut self, span: Span) -> CompileResult<&mut Scope> {
        self.scopes
            .last_mut()
            .ok_or_else(|| CompileError::msg(&span, "missing head of locals"))
    }
}
