use std::collections::HashMap;

use anyhow::{anyhow, Result};
use winnow::{
    ascii::{digit1, multispace0},
    combinator::{alt, delimited, opt, separated, separated_pair, trace},
    error::{ContextError, ErrMode, ParserError},
    stream::{AsChar, Stream, StreamIsPartial},
    token::take_until,
    PResult, Parser,
};

#[derive(Debug, Clone, PartialEq)]
enum Num {
    Int(i64),
    Float(f64),
}

#[derive(Debug, Clone, PartialEq)]
enum JsonValue {
    Null,
    Bool(bool),
    Number(Num),
    String(String),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

fn main() -> Result<()> {
    let s = r#"{
        "name": "John Doe",
        "age": 30,
        "is_student": false,
        "marks": [90.0, -80.0, 85.1],
        "address": {
            "city": "New York",
            "zip": 10001
        }
    }"#;

    let input = &mut (&*s);
    let v = parse_json(input)?;
    println!("{:#?}", v);
    Ok(())
}

fn parse_json(input: &str) -> Result<JsonValue> {
    let input = &mut (&*input);
    parse_value(input).map_err(|e: ErrMode<ContextError>| anyhow!("Failed to parse JSON: {:?}", e))
}

pub fn sep_with_space<Input, Output, Error, ParseNext>(
    mut parser: ParseNext,
) -> impl Parser<Input, (), Error>
where
    Input: Stream + StreamIsPartial,
    <Input as Stream>::Token: AsChar + Clone,
    Error: ParserError<Input>,
    ParseNext: Parser<Input, Output, Error>,
{
    trace("sep_with_space", move |input: &mut Input| {
        let _ = multispace0.parse_next(input)?;
        parser.parse_next(input)?;
        multispace0.parse_next(input)?;
        Ok(())
    })
}

fn parse_null(input: &mut &str) -> PResult<()> {
    "null".value(()).parse_next(input)
}

fn parse_bool(input: &mut &str) -> PResult<bool> {
    alt(("true", "false")).parse_to().parse_next(input)
}

fn parse_num(input: &mut &str) -> PResult<Num> {
    let sign = opt(alt(("+", "-")))
        .map(|s| s.is_some_and(|f| f == "-"))
        .parse_next(input)?;
    let num = digit1.parse_to::<i64>().parse_next(input)?;
    let ret: Result<(), ErrMode<ContextError>> = ".".value(()).parse_next(input);
    if ret.is_ok() {
        let frac = digit1.parse_to::<i64>().parse_next(input)?;
        let mut v = format!("{}.{}", num, frac).parse::<f64>().unwrap();

        let e: Result<(), ErrMode<ContextError>> = alt(("e", "E")).value(()).parse_next(input);
        if e.is_ok() {
            let e_sign = opt(alt(("+", "-")))
                .map(|s| s.is_some_and(|f| f == "-"))
                .parse_next(input)?;
            let exp: i64 = digit1.parse_to::<i64>().parse_next(input)?;
            let exp = if e_sign { -exp } else { exp };
            v *= 10f64.powi(exp as i32);
        }
        Ok(if sign { Num::Float(-v) } else { Num::Float(v) })
    } else {
        Ok(if sign { Num::Int(-num) } else { Num::Int(num) })
    }
}

fn parse_string(input: &mut &str) -> PResult<String> {
    let ret = delimited('"', take_until(0.., '"'), '"').parse_next(input)?;
    PResult::Ok(ret.to_string())
}

fn parse_array(input: &mut &str) -> PResult<Vec<JsonValue>> {
    let sep1 = sep_with_space('[');
    let sep2 = sep_with_space(']');
    let sep_comma = sep_with_space(',');
    let parse_values = separated(0.., parse_value, sep_comma);
    delimited(sep1, parse_values, sep2).parse_next(input)
}

fn parse_object(input: &mut &str) -> PResult<HashMap<String, JsonValue>> {
    let sep1 = sep_with_space('{');
    let sep2 = sep_with_space('}');
    let sep_comma = sep_with_space(',');
    let sep_colon = sep_with_space(':');

    let parse_kv_pair = separated_pair(parse_string, sep_colon, parse_value);
    let parse_kv = separated(1.., parse_kv_pair, sep_comma);
    delimited(sep1, parse_kv, sep2).parse_next(input)
}

fn parse_value(input: &mut &str) -> PResult<JsonValue> {
    alt((
        parse_null.value(JsonValue::Null),
        parse_bool.map(JsonValue::Bool),
        parse_num.map(JsonValue::Number),
        parse_string.map(JsonValue::String),
        parse_array.map(JsonValue::Array),
        parse_object.map(JsonValue::Object),
    ))
    .parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_null() -> PResult<(), ContextError> {
        let input = &mut (r#"null"#);
        parse_null(input)?;

        PResult::Ok(())
    }

    #[test]
    fn test_parse_bool() -> PResult<(), ContextError> {
        let input = &mut (r#"true"#);
        let ret = parse_bool(input)?;
        assert!(ret);

        let input = &mut (r#"false"#);
        let ret = parse_bool(input)?;
        assert!(!ret);

        PResult::Ok(())
    }

    #[test]
    fn test_parse_num() -> PResult<(), ContextError> {
        let input = "123";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Int(123));

        let input = "-123";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Int(-123));

        let input = "123.456";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Float(123.456));

        let input = "-123.456";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Float(-123.456));

        Ok(())
    }

    #[test]
    fn test_parse_string() -> PResult<(), ContextError> {
        let input = &mut (r#""hello""#);
        let ret = parse_string(input)?;
        assert_eq!(ret, "hello");

        PResult::Ok(())
    }

    #[test]
    fn test_parse_array() -> PResult<(), ContextError> {
        let input = r#"[1, 2, 3]"#;
        let result = parse_array(&mut (&*input))?;

        assert_eq!(
            result,
            vec![
                JsonValue::Number(Num::Int(1)),
                JsonValue::Number(Num::Int(2)),
                JsonValue::Number(Num::Int(3))
            ]
        );

        let input = r#"["a", "b", "c"]"#;
        let result = parse_array(&mut (&*input))?;
        assert_eq!(
            result,
            vec![
                JsonValue::String("a".to_string()),
                JsonValue::String("b".to_string()),
                JsonValue::String("c".to_string())
            ]
        );
        Ok(())
    }

    #[test]
    fn test_parse_science_notation() -> PResult<(), ContextError> {
        let input = "1.23e4";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Float(1.23e4));

        let input = "1.23e+4";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Float(1.23e4));

        let input = "1.23e-4";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Float(1.23e-4));

        Ok(())
    }

    #[test]
    fn test_parse_object() -> PResult<(), ContextError> {
        let input = r#"{"a": 1, "b": 2}"#;
        let result = parse_object(&mut (&*input))?;
        let mut expected = HashMap::new();
        expected.insert("a".to_string(), JsonValue::Number(Num::Int(1)));
        expected.insert("b".to_string(), JsonValue::Number(Num::Int(2)));
        assert_eq!(result, expected);

        let input = r#"{"a": 1, "b": [1, 2, 3]}"#;
        let result = parse_object(&mut (&*input))?;
        let mut expected = HashMap::new();
        expected.insert("a".to_string(), JsonValue::Number(Num::Int(1)));
        expected.insert(
            "b".to_string(),
            JsonValue::Array(vec![
                JsonValue::Number(Num::Int(1)),
                JsonValue::Number(Num::Int(2)),
                JsonValue::Number(Num::Int(3)),
            ]),
        );
        assert_eq!(result, expected);

        Ok(())
    }
}
