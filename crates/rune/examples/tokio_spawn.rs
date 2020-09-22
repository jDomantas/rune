#[tokio::main]
async fn main() -> runestick::Result<()> {
    let (context, unit) = rune::testing::build(
        r#"
        async fn main(timeout) {
            time::delay_for(time::Duration::from_secs(timeout)).await
        }
        "#,
    )?;

    let main = runestick::Hash::type_hash(&["main"]);

    let vm = runestick::Vm::new(context.clone(), unit.clone());

    let execution = vm.clone().send_execute(main, (5,))?;
    let t1 = tokio::spawn(async move {
        execution.async_complete().await.unwrap();
        println!("timer ticked");
    });

    let execution = vm.clone().send_execute(main, (2,))?;
    let t2 = tokio::spawn(async move {
        execution.async_complete().await.unwrap();
        println!("timer ticked");
    });

    tokio::try_join!(t1, t2).unwrap();
    Ok(())
}
