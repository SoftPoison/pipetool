# Pipetool

An in-progress tool for chaining different 

## Rough notes

Idea of possible usage:

```sh
# something simple: encoding the input as base64... twice!
echo -ne 'hello!' | pipetool 'in | to_b64 | to_b64 | out'
# ... which should be equivalent to ...
echo -ne 'hello!' | pipetool 'to_b64 | to_b64'

# what about a hex dump?
echo -ne 'hello!' | pipetool '
    fork(
        to_hex | group(2) | fork(
            pass,
            generate(" ")
        ) | merge(alternating) | group(48),

        group(16),

        generate("\n")
    ) | merge(alternating)
'
# .. which should be equivalent to ...
echo -ne 'hello!' | pipetool '
    fork(
        to_hex | group(2) | fork(
            pass,
            generate(" ")
        ) | merge(alternating) | group(48),

        group(16)
    )
'
```

Plan:

```txt
types:
    byte
    char
    num
    stream

in() -> stream<byte>
    returns bytes until EOF

out(input: stream<?>)
    prints the incoming stream to stdout

discard(input: stream<?>)
    discards the incoming stream

fork<T, U>(input: stream<T>, fn0: fn(stream<T>) -> stream<U>, fn1: fn(stream<T>) -> stream<U>, ...) -> stream<stream<U>>
    takes in a stream and more than one mapping function, and outputs a stream of streams of the output type

combine<T>(input: stream<stream<T>>, ?strategy=SEQUENTIAL) -> stream<T>
    combines a stream of streams into a single stream using the provided strategy
    strategies:
        SEQUENTIAL: take all of the first stream, then all of the second stream, and so on
        FOLD: take an element from the first stream, then one from the second stream, and so on, wrapping back around to the first stream

skip<T>(input: stream<T>, count: num) -> stream<T>
    skips the given number of elements

take<T>(input: stream<T>, count: num) -> stream<T>
    takes a maximum of `count` elements from the stream

reverse<T>(input: stream<T>) -> stream<T>
    reverses the input stream

string(input: stream<byte>, ?encoding=UTF8) -> stream<char>
    maps a byte stream into a character stream using the given optional encoding

bytes(input: stream<char>, ?encoding=UTF8) -> stream<byte>
    maps a character stream into a byte stream using the given optional encoding
```