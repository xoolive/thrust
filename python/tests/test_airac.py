from datetime import datetime

from thrust.airac import (
    airac_code_from_date,
    airac_interval,
    effective_date_from_airac_code,
)


def test_airac_roundtrip_python_wrapper() -> None:
    code = airac_code_from_date("2025-08-15")
    assert len(code) == 4

    effective = effective_date_from_airac_code(code)
    begin, end = airac_interval(code)

    assert begin == effective

    begin_dt = datetime.fromisoformat(begin)
    end_dt = datetime.fromisoformat(end)
    date_dt = datetime.fromisoformat("2025-08-15")

    assert begin_dt <= date_dt < end_dt
    assert (end_dt - begin_dt).days == 28


def test_airac_rejects_invalid_code_python_wrapper() -> None:
    try:
        effective_date_from_airac_code("2515")
    except ValueError:
        pass
    else:
        raise AssertionError("expected ValueError for invalid AIRAC code")
