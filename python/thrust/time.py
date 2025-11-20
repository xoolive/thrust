import logging
from datetime import datetime, timedelta, timezone
from numbers import Real
from typing import Any, Union

import pandas as pd  # type: ignore[import]

timelike = Union[str, Real, datetime, pd.Timestamp]
deltalike = Union[None, str, Real, timedelta, pd.Timedelta]

time_or_delta = Union[timelike, timedelta]
timetuple = tuple[datetime, datetime, datetime, datetime]


__all__ = [
    "deltalike",
    "time_or_delta",
    "timelike",
    "timetuple",
    "to_datetime",
    "to_timedelta",
]

_log = logging.getLogger(__name__)


def to_datetime(time: timelike) -> datetime:
    """Facility to convert anything to a datetime.

    This function will soon be replaced by pd.to_datetime.

    >>> f"{to_datetime('2017-01-14')}"
    '2017-01-14 00:00:00+00:00'
    >>> f"{to_datetime('2017-01-14 12:00Z')}"
    '2017-01-14 12:00:00+00:00'
    >>> f"{to_datetime(1484395200)}"
    '2017-01-14 12:00:00+00:00'
    >>> f"{to_datetime(datetime(2017, 1, 14, 12, tzinfo=timezone.utc))}"
    '2017-01-14 12:00:00+00:00'
    """

    if isinstance(time, str):
        time = pd.Timestamp(time, tz="utc")
    if isinstance(time, pd.Timestamp):
        time = time.to_pydatetime()
    if isinstance(time, Real):
        time = datetime.fromtimestamp(float(time), timezone.utc)
    if time.tzinfo is None:  # coverage: ignore
        _log.warning(
            "This timestamp is tz-naive. Things may not work as expected. "
            "If you construct your timestamps manually, consider passing a "
            "string, which defaults to UTC. If you construct your timestamps "
            "automatically, look at the tzinfo (resp. tz) argument of the "
            "datetime (resp. pd.Timestamp) constructor."
        )
    return time  # type: ignore


def to_timedelta(delta: deltalike, **kwargs: Any) -> pd.Timedelta:
    if isinstance(delta, Real):
        delta = pd.Timedelta(seconds=float(delta))
    elif isinstance(delta, (str, timedelta)):
        delta = pd.Timedelta(delta)
    elif delta is None:
        delta = pd.Timedelta(**kwargs)
    return delta
