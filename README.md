
# nixf-diagnose

nixf-diagnose is a CLI wrapper for nixf-tidy with fancy diagnostic output. It provides enhanced error reporting and variable lookup analysis for your source files.


## Usage

To use nixf-diagnose, you need to provide the path to the input source file.
Optionally, you can specify the path to the nixf-tidy executable and enable or disable variable lookup analysis.

nixf-diagnose tries to determine the nixf-tidy path in the following order:

1. Path provided via `--nixf-tidy-path` CLI argument
2. Compile-time constant (embedded during build)
3. Runtime discovery via `which` command


```sh
nixf-diagnose [OPTIONS] [FILES]...
```

Example output:

```
Error: duplicated attrname `a`
   ╭─[nixd/test-workspace/redefined.nix:4:5]
   │
 2 │     a = 1;
   ·     ┬
   ·     ╰── previously declared here
   ·
 4 │     a = 1;
   ·     ┬
   ·     ╰── duplicated attrname `a`
───╯
```

### Options

| Option                                  | Description                                                                    |
| --------------------------------------- | ------------------------------------------------------------------------------ |
| `--nixf-tidy-path <NIXF_TIDY_PATH>`     | Path to the nixf-tidy executable                                               |
| `--variable-lookup [<VARIABLE_LOOKUP>]` | Enable variable lookup analysis [default: true] [possible values: true, false] |
| `-h, --help`                            | Print help                                                                     |
| `-V, --version`                         | Print version                                                                  |
| `-i, --ignore <ID>`                     | Ignore diagnostics with specific ids <br /> This can be used multiple times    |
| `--auto-fix`                            | Automatically apply fixes to source files                                      |


## Author

Yingchi Long <longyingchi24s@ict.ac.cn>

## License

This project is licensed under the MIT License.
