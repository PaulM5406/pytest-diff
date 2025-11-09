#!/usr/bin/env python3
"""Manual test for the parser after rustpython-parser API fix."""

import sys

def test_parser():
    """Test that the parser works with rustpython-parser."""
    try:
        from pytest_diff import _core

        # Test 1: Simple function
        print("Test 1: Simple function")
        source1 = "def foo(): pass"
        blocks1 = _core.parse_module(source1)
        print(f"  Found {len(blocks1)} blocks")
        for block in blocks1:
            print(f"    - {block}")
        assert len(blocks1) >= 2  # module + function
        print("  ✓ PASS\n")

        # Test 2: Class with methods
        print("Test 2: Class with methods")
        source2 = """
class Calculator:
    def add(self, a, b):
        return a + b

    def subtract(self, a, b):
        return a - b
"""
        blocks2 = _core.parse_module(source2)
        print(f"  Found {len(blocks2)} blocks")
        for block in blocks2:
            print(f"    - {block}")
        assert len(blocks2) >= 4  # module + class + 2 methods
        print("  ✓ PASS\n")

        # Test 3: Nested functions
        print("Test 3: Nested functions")
        source3 = """
def outer():
    def inner():
        pass
    return inner
"""
        blocks3 = _core.parse_module(source3)
        print(f"  Found {len(blocks3)} blocks")
        for block in blocks3:
            print(f"    - {block}")
        assert len(blocks3) >= 3  # module + outer + inner
        print("  ✓ PASS\n")

        print("=" * 60)
        print("ALL TESTS PASSED! Parser is working correctly.")
        print("=" * 60)
        return 0

    except Exception as e:
        print(f"ERROR: {e}")
        import traceback
        traceback.print_exc()
        return 1

if __name__ == "__main__":
    sys.exit(test_parser())
