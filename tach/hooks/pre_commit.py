from tach.constants import TOOL_NAME

template = """#!/bin/sh
# Pre-commit script that validates dependencies locally
set -e

{command}"""


def build_pre_commit_hook_content(root: str = "") -> str:
    if root:
        return template.format(command=f"{TOOL_NAME} check --root {root}")
    return template.format(command=f"{TOOL_NAME} check")
