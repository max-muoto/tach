import pytest
import tempfile
import shutil
import os
from modguard import errors
from modguard.init import init_project
from modguard.parsing.boundary import BOUNDARY_PRELUDE


def init_project_from_root(root) -> None:
    # Save the current working directory
    saved_directory = os.getcwd()
    try:
        # Navigate to the root directory and call init_project
        os.chdir(root)
        init_project(root)
    finally:
        # Change back to the original directory
        os.chdir(saved_directory)


@pytest.fixture(scope="module")
def test_root():
    # Create a temporary directory to use as the root for testing
    test_root = tempfile.mkdtemp()
    yield test_root
    # Remove the temporary directory after testing
    shutil.rmtree(test_root)


def test_init_project_with_valid_root(test_root):
    # Create some mock files and directories for testing
    test_dirs = [
        "package1",
        "package2",
        "package3",
        "package4/subpackage",
        "package5/subpackage",
    ]
    for d in test_dirs:
        os.makedirs(os.path.join(test_root, d))
        with open(os.path.join(test_root, d, "__init__.py"), "w") as f:
            f.write("# Mock __init__.py file")

    # Create some mock Python files with imports and member names
    file_contents = {
        "package1/__init__.py": "from package4.subpackage import SubPackageClass\n",
        "package2/__init__.py": "from package5.subpackage import SubPackageClass\n",
        "package3/__init__.py": "from package1.module1 import Package1Class\nfrom package2.module2 import Package2Class\n",
        "package4/subpackage/__init__.py": "",
        "package5/subpackage/__init__.py": "",
        "package1/module1.py": "class Package1Class:\n    pass\n",
        "package2/module2.py": "class Package2Class:\n    pass\n",
    }

    for file_path, content in file_contents.items():
        with open(os.path.join(test_root, file_path), "w") as f:
            f.write(content)

    # Call init_project with the test root
    init_project_from_root(test_root)

    # Check if __init__.py files have been modified as expected
    for d in test_dirs:
        with open(os.path.join(test_root, d, "__init__.py")) as f:
            content = f.read()
            assert BOUNDARY_PRELUDE in content

    # Check if public members have been marked as expected
    expected_public_files = [
        (
            "package1/module1.py",
            "import modguard\n@modguard.public\nclass Package1Class:\n    pass\n",
        ),
        (
            "package2/module2.py",
            "import modguard\n@modguard.public\nclass Package2Class:\n    pass\n",
        ),
    ]
    for file_path, expected_state in expected_public_files:
        with open(os.path.join(test_root, file_path)) as f:
            content = f.read()
            assert content == expected_state


def test_init_project_with_invalid_root():
    with pytest.raises(errors.ModguardSetupError):
        init_project("nonexistent_directory")
