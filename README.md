
# nixf-diagnose

nixf-diagnose is a CLI wrapper for nixf-tidy with fancy diagnostic output. It provides enhanced error reporting and variable lookup analysis for your source files.


## Usage

To use nixf-diagnose, you need to provide the path to the input source file.
Optionally, you can specify the path to the nixf-tidy executable and enable or disable variable lookup analysis.
By default, the `nixf-tidy` command will be looked up in your `$PATH`.
You can install it by installing the `nixf` package from nixpkgs (no need to install the full LSP!).


```sh
./nixf-diagnose --input <path_to_source_file> [--nixf-tidy-path <path_to_nixf_tidy>] [--variable-lookup <true|false>]
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

- `--input, -i <FILE>`: Path to the input source file (required).
- `--nixf-tidy-path <PATH>`: Path to the nixf-tidy executable (optional).
- `--variable-lookup <BOOL>`: Enable or disable variable lookup analysis (default: true).

### Example

```sh
./nixf-diagnose --input package.nix --variable-lookup false
```

This command runs nixf-diagnose on `package.nix` with variable lookup analysis disabled.

## Author

Yingchi Long <longyingchi24s@ict.ac.cn>

## License

This project is licensed under the MIT License.
