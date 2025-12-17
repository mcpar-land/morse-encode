# Morse Encode

Toy CLI tool for reading and writing to a binary format that reflects morse code.

By default, pipe any UTF-8 stream into it to get an output in a morse code-style, unaligned byte encoding.

```sh
morse-encode < sample.txt > sample.morse

some_command | morse-encode > sample.morse
```

To decode, pipe morse-encoded bytes in and pass the `--decode`/`-d` flag.

```sh
morse-encode -d < sample.morse

some_command | morse-encode | morse-encode -d
```

You can replace newlines and periods with the classic **STOP** often used in telegrams by passing `--stop`/`-s`

```sh 
echo "Hello. I am sending you this message." | morse-encode | morse-encode -d
```
```
HELLO I AM SENDING YOU THIS MESSAGE
```

```sh
echo "Hello. I am sending you this message." | morse-encode -s | morse-encode -d
```
```
HELLO STOP I AM SENDING YOU THIS MESSAGE STOP
```

## Format details

Based on the international morse code found [here](https://commons.wikimedia.org/wiki/File:International_Morse_Code.svg).

The length of units is changed to preserve size.

| Signal                                 | Length |
|----------------------------------------|--------|
| Dot                                    | 1      |
| Dash                                   | 2      |
| Space between parts of the same letter | 1      |
| Space between letters                  | 2      |
| Space between words                    | 3      |
