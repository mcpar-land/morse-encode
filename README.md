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

| Signal                                 | Bits       |
|----------------------------------------|------------|
| Dot                                    | `1`        |
| Dash                                   | `11`       |
| Space between parts of the same letter | `0`        |
| Space between letters                  | `00`       |
| Space between words                    | `000`      |

So sending the letter S, which is three dots, would be `10101`.

"HELLO" would be:

| Character | Representation | Bits       |
|-----------|----------------|------------|
| H         | `....`         | `1010101`  |
| E         | `.`            | `1`        |
| L         | `.-..`         | `10110101` |
| L         | `.-..`         | `10110101` |
| O         | `---`          | `11011011` |

This translates into:

```  
1010101  1  10110101  10110101  11011011
1010101001001011010100101101010011011011
```

Arranged in bytes, it looks like:
```
10101010 01001011 01010010 11010100 11011011
aa       4b       52       d4       db
```

These bits are written directly, and are not aligned to bytes in any way. Trailing zeroes are used in the last byte of the message as necessary (between 1 and 7)

For example, encoding "HELLOE" shows the trailing bytes:

```
10101010 01001011 01010010 11010100 11011011 00100000
```
