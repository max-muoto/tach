[![image](https://img.shields.io/pypi/v/tach.svg)](https://pypi.Python.org/pypi/tach)
[![image](https://img.shields.io/pypi/l/tach.svg)](https://pypi.Python.org/pypi/tach)
[![image](https://img.shields.io/pypi/pyversions/tach.svg)](https://pypi.Python.org/pypi/tach)
[![image](https://github.com/gauge-sh/tach/actions/workflows/ci.yml/badge.svg)](https://github.com/gauge-sh/tach/actions/workflows/ci.yml)
[![Checked with pyright](https://microsoft.github.io/pyright/img/pyright_badge.svg)](https://microsoft.github.io/pyright/)
[![Ruff](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/astral-sh/ruff/main/assets/badge/v2.json)](https://github.com/astral-sh/ruff)
# tach
a Python tool to enforce modular design


[Docs](https://gauge-sh.github.io/tach/)

[Discord](https://discord.gg/DKVksRtuqS) - come say hi!



https://github.com/gauge-sh/tach/assets/10570340/2f5ed866-124e-4322-afe6-15207727ca38



## What is tach?
`tach` allows you to define boundaries and control dependencies between your Python packages. Each package can also define its public interface.

This enforces a decoupled, modular architecture, and prevents tight coupling.
If a package tries to import from another package that is not listed as a dependency, tach will report an error.
If a package tries to import from another package and does not use its public interface, with `strict: true` set, `tach` will report an error.

`tach` is incredibly lightweight, and has no impact on your runtime. Instead, its checks are performed as a lint check through the CLI.

## Installation
```bash
pip install tach
```

## Quickstart
`tach` comes bundled with a command to interactively define your package boundaries.
Run the following in the root of your Python project to enter the editor:
```bash
tach pkg
```

The interactive editor allows you to mark which directories should be treated as package boundaries.
You can navigate with the arrow keys, mark individual packages with `Enter`, and mark all sibling directories
as packages with `Ctrl + a`.

After identifying your packages, press `Ctrl + s` to initialize the boundaries.
Each package will receive a `package.yml` with a single tag based on the folder name,
and a default `tach.yml` file will be created in the current working directory.

If you want to sync your `tach.yml` with the actual dependencies found in your project, you can use `tach sync`:
```bash
tach sync [--prune]
```

Any dependency errors will be automatically resolved by
adding the corresponding dependencies to your `tach.yml` file. If you supply `--prune`,
any dependency constraints in your `tach.yml` which are not necessary will also be removed.

In case you want to start over, `tach clean` lets you delete all `tach` configuration files so that you can re-initialize or configure your packages manually.
```bash
tach clean
```


## Defining Packages
To define a package, add a `package.yml` to the corresponding Python package. Add at least one 'tag' to identify the package.

Examples:
```python
# core/package.yml
tags: ["core"]
```
```python
# db/package.yml
tags: ["db"]
```
```python
# utils/package.yml
tags: ["utils"]
```
Next, specify the constraints for each tag in `tach.yml` in the root of your project:
```yaml
# [root]/tach.yml
constraints:
- tag: core
  depends_on:
  - db
  - utils
- tag: db
  depends_on:
  - utils
- tag: utils
  depends_on: []
```
With these rules in place, packages with tag `core` can import from packages with tag `db` or `utils`. Packages tagged with `db` can only import from `utils`, and packages tagged with `utils` cannot import from any other packages in the project. 

`tach` will now flag any violation of these boundaries.
```bash
# From the root of your Python project (in this example, `project/`)
> tach check
❌ utils/helpers.py[L10]: Cannot import 'core.PublicAPI'. Tags ['utils'] cannot depend on ['core'].
```

NOTE: If your terminal supports hyperlinks, you can click on the failing file path to go directly to the error.

## Defining Interfaces
If you want to define a public interface for the package, import and reference each object you want exposed in the package's `__init__.py` and add its name to `__all__`:
```python
# db/__init__.py
from db.service import PublicAPI

__all__ = ["PublicAPI"]
```
Turning on `strict: true` in the package's `package.yml` will then enforce that all imports from this package occur through `__init__.py` and are listed in `__all__`
```yaml
# db/package.yml
tags: ["db"]
strict: true
```
```python
# The only valid import from "db"
from db import PublicAPI 
```

### Pre-Commit Hook
`tach` can be installed as a pre-commit hook. See the [docs](https://gauge-sh.github.io/tach/usage/#tach-install) for installation instructions.


## Advanced
`tach` supports specific exceptions. You can mark an import with the `tach-ignore` comment:
```python
# tach-ignore
from db.main import PrivateAPI
```
This will stop `tach` from flagging this import as a boundary violation.

You can also specify multiple tags for a given package:
```python
# utils/package.yml
tags: ["core", "utils"]
```
This will expand the set of packages that "utils" can access to include all packages that "core" and "utils" `depends_on` as defined in `tach.yml`.

By default, `tach` ignores hidden directories and files (paths starting with `.`). To override this behavior, set `exclude_hidden_paths` in `tach.yml`
```yaml
exclude_hidden_paths: false
```

## Details
`tach` works by analyzing the abstract syntax tree (AST) of your codebase. It has no runtime impact, and all operations are performed statically. 

Boundary violations are detected at the import layer. This means that dynamic imports using `importlib` or similar approaches will not be caught by tach.

[PyPi Package](https://pypi.org/project/tach/)

### License
[GNU GPLv3](LICENSE)
