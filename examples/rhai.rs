use rhai::{Engine, EvalAltResult};

fn main() -> Result<(), Box<EvalAltResult>> {
    let engine = Engine::new();
    engine.run(r#"print("hello world!")"#)?;
    let result = engine.eval::<i64>("40 + 2")?;
    println!("The Answer: {result}");
    Ok(())
}
