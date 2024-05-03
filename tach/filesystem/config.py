import os
import sys
from pathlib import Path
from typing import Optional

from tach.colors import BCOLORS
from tach.constants import CONFIG_FILE_NAME, TOML_CONFIG_FILE_NAME


def print_no_config_yml() -> None:
    print(
        f"{BCOLORS.FAIL} {CONFIG_FILE_NAME}.(yml|yaml) not found in {os.getcwd()}{BCOLORS.ENDC}",
        file=sys.stderr,
    )


def get_project_config_yml_path(root: str = ".") -> str:
    file_path = os.path.join(root, f"{CONFIG_FILE_NAME}.yml")
    if os.path.exists(file_path):
        return file_path
    file_path = os.path.join(root, f"{CONFIG_FILE_NAME}.yaml")
    if os.path.exists(file_path):
        return file_path
    return ""


def find_project_config_yml_root(path: str) -> Optional[str]:
    path = os.path.abspath(path)
    if os.path.isdir(path):
        if get_project_config_yml_path(path):
            return path
    path_obj = Path(path)
    # Iterate upwards, looking for project config
    for parent in path_obj.parents:
        if get_project_config_yml_path(str(parent)):
            return str(parent)


def validate_project_config_yml_path(root: str = ".") -> str:
    project_config_path = get_project_config_yml_path(root)
    if not project_config_path:
        print_no_config_yml()
        sys.exit(1)
    else:
        return project_config_path


def get_toml_config_path(root: str = ".") -> str:
    toml_config_path = os.path.join(root, TOML_CONFIG_FILE_NAME)
    if os.path.exists(toml_config_path):
        return toml_config_path
    return ""


def find_toml_config(path: str = ".") -> Optional[str]:
    path = os.path.abspath(path)
    if os.path.isdir(path):
        toml_config_path = get_toml_config_path(path)
        if toml_config_path:
            return toml_config_path
    path_obj = Path(path)
    # Iterate upwards, looking for project config
    for parent in path_obj.parents:
        if get_toml_config_path(str(parent)):
            return str(parent / TOML_CONFIG_FILE_NAME)


def find_project_root(path: str = ".") -> Optional[str]:
    project_config_yml = find_project_config_yml_root(path)
    if project_config_yml:
        return project_config_yml
    project_config_toml = find_toml_config(path)
    if project_config_toml:
        return os.path.dirname(project_config_toml)


def append_to_toml(toml_config_path: str, content: str) -> None:
    with open(toml_config_path, "a") as f:
        f.write("\n" + content)
