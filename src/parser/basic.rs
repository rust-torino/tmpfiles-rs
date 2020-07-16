use nom::character::complete::char;
use nom::IResult;


pub fn empty_placeholder(input: &[u8]) -> IResult<&[u8], char> {
    char('-')(input)
}