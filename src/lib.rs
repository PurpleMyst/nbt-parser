#[macro_use]
extern crate combine;

extern crate libflate;

extern crate failure;

use combine::parser::byte::{
    byte,
    num::{be_f32, be_f64, be_i16, be_i32, be_i64, be_u16},
};
use combine::stream::{buffered::BufferedStream, state::State, ReadStream};
use combine::{any, count, many, unexpected};
use combine::{ParseError, Parser, Stream};

use std::{io::Read, mem};

/// An unnamed tag.
#[derive(Clone, Debug, PartialEq)]
pub enum UnnamedTag {
    /// The `TAG_End` tag. Normally not found anywhere but inside `TAG_Compound`s.
    End,

    /// The `TAG_Byte` tag.
    Byte(i8),

    /// The `TAG_Short` tag.
    Short(i16),

    /// The `TAG_Int` tag.
    Int(i32),

    /// The `TAG_Long` tag.
    Long(i64),

    /// The `TAG_Float` tag.
    Float(f32),

    /// The `TAG_Double` tag.
    Double(f64),

    /// The `TAG_ByteArray` tag.
    ByteArray(Vec<i8>),

    /// The `TAG_String` tag.
    String(String),

    /// The `TAG_List` tag. This contains unnamed tags, which are guaranteed to be all of the same
    /// type.
    List(Vec<UnnamedTag>),

    /// The `TAG_Compound` tag. This contains named tags, but the `TAG_End` tag which is always
    /// present at the end is removed for ease of use.
    Compound(Vec<NamedTag>),
}

/// A named tag. Contains only the name on its own, and the actual tag's contents are accessible
/// via the `content` field.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedTag {
    /// The name of the tag. This is the empty string if the inner tag is a `TAG_End`.
    pub name: String,
    /// The actual content of the tag.
    pub content: UnnamedTag,
}

fn name<I>() -> impl Parser<Input = I, Output = String>
where
    I: Stream<Item = u8>,
    // Necessary due to rust-lang/rust#24159
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    be_u16()
        .then(|length| count(length as usize, any()))
        .map(|contents: Vec<u8>| String::from_utf8(contents).unwrap())
}

fn end_tag<I>() -> impl Parser<Input = I, Output = NamedTag>
where
    I: Stream<Item = u8>,
    // Necessary due to rust-lang/rust#24159
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    byte(0).map(|_| NamedTag {
        name: String::new(),
        content: UnnamedTag::End,
    })
}

fn i8<I>() -> impl Parser<Input = I, Output = i8>
where
    I: Stream<Item = u8>,
    // Necessary due to rust-lang/rust#24159
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    any().map(|n| unsafe { mem::transmute::<u8, i8>(n) })
}

macro_rules! simple_number_tag {
    ($func_name:ident, $parser_name:ident, $tag_variant:path) => {
        fn $func_name<I>() -> impl Parser<Input = I, Output = UnnamedTag>
        where
            I: Stream<Item = u8>,
            // Necessary due to rust-lang/rust#24159
            I::Error: ParseError<I::Item, I::Range, I::Position>,
        {
            $parser_name().map($tag_variant)
        }
    };
}

simple_number_tag!(byte_tag, i8, UnnamedTag::Byte);
simple_number_tag!(short_tag, be_i16, UnnamedTag::Short);
simple_number_tag!(int_tag, be_i32, UnnamedTag::Int);
simple_number_tag!(long_tag, be_i64, UnnamedTag::Long);
simple_number_tag!(float_tag, be_f32, UnnamedTag::Float);
simple_number_tag!(double_tag, be_f64, UnnamedTag::Double);

fn bytearray_tag<I>() -> impl Parser<Input = I, Output = UnnamedTag>
where
    I: Stream<Item = u8>,
    // Necessary due to rust-lang/rust#24159
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    be_i32()
        .then(|length| count(length as usize, i8()))
        .map(UnnamedTag::ByteArray)
}

fn string_tag<I>() -> impl Parser<Input = I, Output = UnnamedTag>
where
    I: Stream<Item = u8>,
    // Necessary due to rust-lang/rust#24159
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    name().map(UnnamedTag::String)
}

fn list_tag<I>() -> impl Parser<Input = I, Output = UnnamedTag>
where
    I: Stream<Item = u8>,
    // Necessary due to rust-lang/rust#24159
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    i8().and(be_i32())
        .then(|(tag_id, length)| {
            count(
                length as usize,
                combine::parser(move |input| match tag_id {
                    0 => end_tag()
                        .map(|NamedTag { content, .. }| content)
                        .parse_stream(input),
                    1 => byte_tag().parse_stream(input),
                    2 => short_tag().parse_stream(input),
                    3 => int_tag().parse_stream(input),
                    4 => long_tag().parse_stream(input),
                    5 => float_tag().parse_stream(input),
                    6 => double_tag().parse_stream(input),
                    7 => bytearray_tag().parse_stream(input),
                    8 => string_tag().parse_stream(input),
                    9 => list_tag().parse_stream(input),
                    10 => compound_tag().parse_stream(input),
                    _ => unexpected("Invalid tagId on TAG_List")
                        .map(|()| UnnamedTag::End)
                        .parse_stream(input),
                }),
            )
        })
        .map(UnnamedTag::List)
}

fn compound_tag<I>() -> impl Parser<Input = I, Output = UnnamedTag>
where
    I: Stream<Item = u8>,
    // Necessary due to rust-lang/rust#24159
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    (many(combine::parser(|input| {
        let (tag, rest) = named_tag().parse_stream(input)?;
        if tag.content == UnnamedTag::End {
            combine::unexpected("If you see this, contact github.com/PurpleMyst")
                .map(|_| tag.clone())
                .parse_stream(input)
        } else {
            Ok((tag, rest))
        }
    }))).skip(end_tag())
        .map(UnnamedTag::Compound)
}

fn named_tag<I>() -> impl Parser<Input = I, Output = NamedTag>
where
    I: Stream<Item = u8>,
    // Necessary due to rust-lang/rust#24159
    I::Error: ParseError<I::Item, I::Range, I::Position>,
{
    macro_rules! do_it {
        ($num:expr => $parser:expr) => {
            byte($num)
                .with(name())
                .and($parser)
                .map(|(name, content)| NamedTag { name, content })
        };
    }

    choice!(
        end_tag(),
        do_it!(1 => byte_tag()),
        do_it!(2 => short_tag()),
        do_it!(3 => int_tag()),
        do_it!(4 => long_tag()),
        do_it!(5 => float_tag()),
        do_it!(6 => double_tag()),
        do_it!(7 => bytearray_tag()),
        do_it!(8 => string_tag()),
        do_it!(9 => list_tag()),
        do_it!(10 => compound_tag())
    )
}

/// Decode a [`Read`] instance. It is assumed that, as the spec defines, the contents of the
/// instance are gzipped.
pub fn decode<R: Read>(mut input: R) -> Result<NamedTag, failure::Error> {
    let decoder = libflate::gzip::Decoder::new(&mut input)?;
    decode_uncompressed(decoder)
}

/// Decode a [`Read`] instance. It is assumed that the contents of the instance are not gzipped.
pub fn decode_uncompressed<R: Read>(input: R) -> Result<NamedTag, failure::Error> {
    let mut stream = BufferedStream::new(State::new(ReadStream::new(input)), 4096);
    Ok(named_tag().parse_stream(&mut stream).map_err(|c| c.into_inner().error)?.0)
}
