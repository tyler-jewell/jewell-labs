def sum_range(lo: int, hi: int) -> int:
    """Sum integers from lo to hi inclusive."""
    total = 0
    x = lo
    while x <= hi:
        total += x
        x += 1
    return total
