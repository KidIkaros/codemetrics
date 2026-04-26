"""Sample Python module for testing."""

import os
import sys
from typing import List

def helper(x: int) -> int:
    """A helper function."""
    return x * 2

def process(items: List[int]) -> int:
    # TODO: add error handling
    total = 0
    for item in items:
        total += helper(item)
    return total

def unused(a: str, b: str) -> str:
    return a + b
