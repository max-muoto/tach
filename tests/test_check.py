import pytest
from modguard.check import check, file_to_module_path, has_boundary, get_imports


def test_file_to_mod_path():
    assert file_to_module_path("__init__.py") == ""
    assert file_to_module_path("domain_one/__init__.py") == "domain_one"
    assert file_to_module_path("domain_one/interface.py") == "domain_one.interface"


def test_has_boundary():
    assert has_boundary("example/domain_one/__init__.py")
    assert not has_boundary("example/domain_one/interface.py")


def test_get_imports():
    assert get_imports("example/domain_one/interface.py") == ["modguard.public"]
    assert get_imports("example/domain_one/__init__.py") == [
        "modguard.Boundary",
        "example.domain_one.interface.domain_one_interface",
    ]
    assert get_imports("example/__init__.py") == [
        "modguard.Boundary",
        "example.domain_one.interface.domain_one_interface",
    ]


def test_check():
    expected_errors = 2
    check_results = check("example")
    assert len(check_results) == expected_errors, "\n".join(
        (result.message for result in check_results)
    )
    check_results = check(".")
    assert len(check_results) == expected_errors, "\n".join(
        (result.message for result in check_results)
    )