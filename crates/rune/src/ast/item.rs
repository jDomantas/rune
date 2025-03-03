use crate::ast::prelude::*;

/// A declaration.
#[derive(Debug, Clone, PartialEq, Eq, ToTokens, Spanned)]
#[non_exhaustive]
pub enum Item {
    /// A use declaration.
    Use(ast::ItemUse),
    /// A function declaration.
    // large variant, so boxed
    Fn(ast::ItemFn),
    /// An enum declaration.
    Enum(ast::ItemEnum),
    /// A struct declaration.
    Struct(ast::ItemStruct),
    /// An impl declaration.
    Impl(ast::ItemImpl),
    /// A module declaration.
    Mod(ast::ItemMod),
    /// A const declaration.
    Const(ast::ItemConst),
    /// A macro call expanding into an item.
    MacroCall(ast::MacroCall),
}

impl Item {
    /// Test if the item has any attributes
    pub(crate) fn attributes(&self) -> &[ast::Attribute] {
        match self {
            Self::Use(item) => &item.attributes,
            Self::Fn(item) => &item.attributes,
            Self::Enum(item) => &item.attributes,
            Self::Struct(item) => &item.attributes,
            Self::Impl(item) => &item.attributes,
            Self::Mod(item) => &item.attributes,
            Self::Const(item) => &item.attributes,
            Self::MacroCall(item) => &item.attributes,
        }
    }

    /// Indicates if the declaration needs a semi-colon or not.
    pub(crate) fn needs_semi_colon(&self) -> bool {
        match self {
            Self::Use(..) => true,
            Self::Struct(st) => st.needs_semi_colon(),
            Self::Const(..) => true,
            _ => false,
        }
    }

    /// Test if declaration is suitable inside of a file.
    pub(crate) fn peek_as_item(p: &mut Peeker<'_>) -> bool {
        match p.nth(0) {
            K![use] => true,
            K![enum] => true,
            K![struct] => true,
            K![impl] => true,
            K![async] => matches!(p.nth(1), K![fn]),
            K![fn] => true,
            K![mod] => true,
            K![const] => true,
            _ => false,
        }
    }

    /// Parse an Item attaching the given meta and optional path.
    pub(crate) fn parse_with_meta_path(
        p: &mut Parser<'_>,
        mut attributes: Vec<ast::Attribute>,
        mut visibility: ast::Visibility,
        path: Option<ast::Path>,
    ) -> Result<Self, ParseError> {
        use std::mem::take;

        let item = if let Some(path) = path {
            Self::MacroCall(ast::MacroCall::parse_with_meta_path(
                p,
                take(&mut attributes),
                path,
            )?)
        } else {
            let mut const_token = p.parse::<Option<T![const]>>()?;
            let mut async_token = p.parse::<Option<T![async]>>()?;

            let item = match p.nth(0)? {
                K![use] => Self::Use(ast::ItemUse::parse_with_meta(
                    p,
                    take(&mut attributes),
                    take(&mut visibility),
                )?),
                K![enum] => Self::Enum(ast::ItemEnum::parse_with_meta(
                    p,
                    take(&mut attributes),
                    take(&mut visibility),
                )?),
                K![struct] => Self::Struct(ast::ItemStruct::parse_with_meta(
                    p,
                    take(&mut attributes),
                    take(&mut visibility),
                )?),
                K![impl] => Self::Impl(ast::ItemImpl::parse_with_attributes(
                    p,
                    take(&mut attributes),
                )?),
                K![fn] => Self::Fn(ast::ItemFn::parse_with_meta(
                    p,
                    take(&mut attributes),
                    take(&mut visibility),
                    take(&mut const_token),
                    take(&mut async_token),
                )?),
                K![mod] => Self::Mod(ast::ItemMod::parse_with_meta(
                    p,
                    take(&mut attributes),
                    take(&mut visibility),
                )?),
                K![ident] => {
                    if let Some(const_token) = const_token.take() {
                        Self::Const(ast::ItemConst::parse_with_meta(
                            p,
                            take(&mut attributes),
                            take(&mut visibility),
                            const_token,
                        )?)
                    } else {
                        Self::MacroCall(p.parse()?)
                    }
                }
                _ => {
                    return Err(ParseError::expected(
                        p.tok_at(0)?,
                        "`fn`, `mod`, `struct`, `enum`, `use`, or macro call",
                    ))
                }
            };

            if let Some(span) = const_token.option_span() {
                return Err(ParseError::unsupported(span, "const modifier"));
            }

            if let Some(span) = async_token.option_span() {
                return Err(ParseError::unsupported(span, "async modifier"));
            }

            item
        };

        if let Some(span) = attributes.option_span() {
            return Err(ParseError::unsupported(span, "attribute"));
        }

        if let Some(span) = visibility.option_span() {
            return Err(ParseError::unsupported(span, "visibility modifier"));
        }

        Ok(item)
    }
}

impl Parse for Item {
    fn parse(p: &mut Parser) -> Result<Self, ParseError> {
        let attributes = p.parse()?;
        let visibility = p.parse()?;
        let path = p.parse()?;
        Self::parse_with_meta_path(p, attributes, visibility, path)
    }
}
