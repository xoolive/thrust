from __future__ import annotations

from datetime import datetime, timezone
from typing import Any, Iterator, Literal

import pandas as pd  # type: ignore[import]

from .core.intervals import (
    collection_add,
    collection_addi,
    collection_and,
    collection_andi,
    collection_sub,
    collection_subi,
    interval_add,
    interval_and,
    interval_sub,
)
from .time import timelike, to_datetime


def _to_epoch_seconds(series: pd.Series) -> Any:
    values = series.astype("int64").to_numpy()
    unit = getattr(series.dtype, "unit", "ns")
    scale = {"ns": 1_000_000_000, "us": 1_000_000, "ms": 1_000, "s": 1}.get(
        unit, 1_000_000_000
    )
    return values // scale


class Interval:
    start: datetime
    stop: datetime

    def __init__(self, start: float | datetime, stop: float | datetime) -> None:
        start = (
            datetime.fromtimestamp(start, timezone.utc)
            if isinstance(start, (int, float))
            else start
        )
        stop = (
            datetime.fromtimestamp(stop, timezone.utc)
            if isinstance(stop, (int, float))
            else stop
        )
        self.start = start
        self.stop = stop
        if self.start > self.stop:
            raise RuntimeError("Start value should be anterior to stop value")

    def __repr__(self) -> str:
        return f"[{self.start}, {self.stop}]"

    def __eq__(self, other: Any) -> bool:
        if not isinstance(other, Interval):
            return False
        return self.start == other.start and self.stop == other.stop

    def __and__(self, other: Interval) -> None | Interval:
        """Returns the intersection between two intervals.

        >>> i1 = Interval(1647861000, 1647861120)
        >>> i2 = Interval(1647861060, 1647861180)
        >>> i1 & i2
        [2022-03-21 11:11:00+00:00, 2022-03-21 11:12:00+00:00]

        """

        if isinstance(other, Interval):
            res = interval_and(
                int(self.start.timestamp()),
                int(self.stop.timestamp()),
                int(other.start.timestamp()),
                int(other.stop.timestamp()),
            )
            # print(self, other, res)
            if res.get("empty", None):
                return None
            return Interval(res["start"], res["stop"])

        return NotImplemented

    def __add__(self, other: Interval) -> IntervalCollection:
        """Concatenates the two elements in an IntervalCollection.

        >>> i1 = Interval(1647861000, 1647861120)
        >>> i2 = Interval(1647861060, 1647861180)
        >>> i1 + i2
        [[2022-03-21 11:10:00+00:00, 2022-03-21 11:13:00+00:00]]

        """
        if isinstance(other, Interval):
            res = interval_add(
                int(self.start.timestamp()),
                int(self.stop.timestamp()),
                int(other.start.timestamp()),
                int(other.stop.timestamp()),
            )
            return IntervalCollection(list(res["start"]), list(res["stop"]))

        return NotImplemented

    def __sub__(self, other: Interval) -> None | IntervalCollection:
        """Concatenates the two elements in an IntervalCollection.

        >>> i1 = Interval(1647861000, 1647861120)
        >>> i2 = Interval(1647861060, 1647861180)
        >>> i1 - i2
        [[2022-03-21 11:10:00+00:00, 2022-03-21 11:11:00+00:00]]
        >>> i3 = Interval(1647861240, 1647861300)
        >>> i1 - i3
        [[2022-03-21 11:10:00+00:00, 2022-03-21 11:12:00+00:00]]

        """
        if isinstance(other, Interval):
            res = interval_sub(
                int(self.start.timestamp()),
                int(self.stop.timestamp()),
                int(other.start.timestamp()),
                int(other.stop.timestamp()),
            )
            if len(res["start"]) == 0:
                return None
            return IntervalCollection(list(res["start"]), list(res["stop"]))

        return NotImplemented

    def duration(self) -> pd.Timedelta:
        return self.stop - self.start

    def overlap(self, other: Interval) -> bool:
        """Returns True if two intervals overlap."""
        return self.start <= other.stop and self.stop >= other.start


class IntervalCollection:
    """A class to represent collections of Intervals.

    An :class:`~Interval` consists of a start and stop attributes.
    Collections of intervals are stored as a :class:`~pandas.DataFrame`.

    Intervals can be created using one of the following syntaxes:

    >>> sample_dates = pd.date_range("2023-01-01", "2023-02-01", freq="1D")
    >>> t0, t1, t2, t3, *_ = sample_dates

    - as a list of :class:`~Interval`:

        >>> IntervalCollection([Interval(t0, t1), Interval(t2, t3)])
        [[2023-01-01 00:00:..., 2023-01-02 00:00:...], ...]

    - as an expanded tuple of :class:`~Interval`:

        >>> IntervalCollection(Interval(t0, t1), Interval(t2, t3))
        [[2023-01-01 00:00:..., 2023-01-02 00:00:...], ...]

    - a list of start and stop values:

        >>> IntervalCollection([t0, t2], [t1, t3])
        [[2023-01-01 00:00:..., 2023-01-02 00:00:...], ...]

    - as a :class:`~pandas.DataFrame`:

        >>> df = pd.DataFrame({'start': [t0, t2], 'stop': [t1, t3]})
        >>> IntervalCollection(df)
        [[2023-01-01 00:00:..., 2023-01-02 00:00:...], ...]

    """

    data: pd.DataFrame

    def __init__(
        self,
        data: None
        | pd.DataFrame
        | Interval
        | list[Interval]
        | timelike
        | list[timelike] = None,
        *other: Interval | timelike | list[timelike],
        start: None | timelike | list[timelike] = None,
        stop: None | timelike | list[timelike] = None,
    ) -> None:
        if isinstance(data, Interval):
            data = [data, *other]
        if isinstance(data, list):
            if all(isinstance(elt, Interval) for elt in data):
                if len(data) == 1:
                    data = pd.DataFrame.from_records(
                        [{"start": data[0].start, "stop": data[0].stop}]
                    )
                else:
                    res = sum(data[1:], data[0])
                    data = res.data  # type: ignore
        if not isinstance(data, pd.DataFrame):
            # We reorder parameters here to accept notations like
            # IntervalCollection(start, stop)
            if start is None or stop is None:
                start, stop, *_ = data, *other, start, stop
                data = None
        if data is None:
            if start is None or stop is None:
                msg = "If no data is specified, provide start and stop"
                raise TypeError(msg)
            if isinstance(start, (str, float, datetime, pd.Timestamp)):
                start = [start]
            if isinstance(stop, (str, float, datetime, pd.Timestamp)):
                stop = [stop]
            assert isinstance(start, list)
            assert isinstance(stop, list)
            if len(start) == 0 or len(stop) == 0:
                msg = "If no data is specified, provide start and stop"
                raise TypeError(msg)

            data = pd.DataFrame(
                {
                    "start": [to_datetime(t) for t in start],
                    "stop": [to_datetime(t) for t in stop],
                }
            )

        # assert isinstance(data, pd.DataFrame)
        # assert data.eval("(start > stop).sum()") == 0

        self.data = data

    def __iter__(self) -> Iterator[Interval]:
        for _, line in self.data.iterrows():
            yield Interval(line.start, line.stop)

    def __repr__(self) -> str:
        return repr(list(i for i in self))

    def __eq__(self, other: Any) -> bool:
        if isinstance(other, IntervalCollection):
            if self.data.shape != other.data.shape:
                return False
            left = self.data.sort_values(by=["start"], ignore_index=True)
            right = other.data.sort_values(by=["start"], ignore_index=True)
            left_start = _to_epoch_seconds(left["start"])
            left_stop = _to_epoch_seconds(left["stop"])
            right_start = _to_epoch_seconds(right["start"])
            right_stop = _to_epoch_seconds(right["stop"])
            return (left_start == right_start).all() and (
                left_stop == right_stop
            ).all()
        return False

    def __radd__(self, other: Literal[0] | Interval) -> IntervalCollection:
        if other == 0:
            return self
        return IntervalCollection(other) + self

    def __add__(
        self, other: Interval | IntervalCollection
    ) -> IntervalCollection:
        """
        >>> i1 = Interval(1647861000, 1647861120)
        >>> i2 = Interval(1647861060, 1647861180)
        >>> i3 = Interval(1647861240, 1647861300)
        >>> (i1 + i2) + i3
        [[... 11:10..., ... 11:13...], [... 11:14..., ... 11:15:...]]
        >>> i1 + (i2 + i3)
        [[... 11:10..., ... 11:13...], [... 11:14..., ... 11:15:...]]
        """
        if isinstance(other, Interval):
            res = collection_addi(
                _to_epoch_seconds(self.data.start),
                _to_epoch_seconds(self.data.stop),
                int(other.start.timestamp()),
                int(other.stop.timestamp()),
            )
            data = pd.DataFrame({"start": res["start"], "stop": res["stop"]})
            data = data.eval(
                """start = @pd.to_datetime(start, unit="s", utc=True)
                   stop = @pd.to_datetime(stop, unit="s", utc=True)"""
            )
            return IntervalCollection(data)

        if isinstance(other, IntervalCollection):
            res = collection_add(
                _to_epoch_seconds(self.data.start),
                _to_epoch_seconds(self.data.stop),
                _to_epoch_seconds(other.data.start),
                _to_epoch_seconds(other.data.stop),
            )
            data = pd.DataFrame({"start": res["start"], "stop": res["stop"]})
            data = data.eval(
                """start = @pd.to_datetime(start, unit="s", utc=True)
                   stop = @pd.to_datetime(stop, unit="s", utc=True)"""
            )
            return IntervalCollection(data)

        return NotImplemented

    def __rsub__(self, other: Interval) -> None | IntervalCollection:
        return IntervalCollection(other) - self

    def __sub__(
        self, other: Interval | IntervalCollection
    ) -> None | IntervalCollection:
        """
        >>> i1 = Interval(1647861000, 1647861120)
        >>> i2 = Interval(1647861060, 1647861180)
        >>> i3 = Interval(1647861240, 1647861300)
        >>> i1 + i3
        [[... 11:10..., ... 11:12...], [... 11:14..., ... 11:15...]]
        >>> (i1 + i2 + i3)
        [[... 11:10..., ... 11:13...], [... 11:14..., ... 11:15...]]
        >>> (i1 + i2 + i3) - i3
        [[... 11:10..., ... 11:13...]]
        >>> (i1 + i2 + i3) - (i3 + i2)
        [[... 11:10..., ... 11:11...]]
        >>> (i1 + i2) - (i3 + i2)
        [[... 11:10..., ... 11:11...]]
        """
        if isinstance(other, Interval):
            res = collection_subi(
                _to_epoch_seconds(self.data.start),
                _to_epoch_seconds(self.data.stop),
                int(other.start.timestamp()),
                int(other.stop.timestamp()),
            )
            if len(res["start"]) == 0:
                return None
            data = pd.DataFrame({"start": res["start"], "stop": res["stop"]})
            data = data.eval(
                """start = @pd.to_datetime(start, unit="s", utc=True)
                   stop = @pd.to_datetime(stop, unit="s", utc=True)"""
            )
            return IntervalCollection(data)

        if isinstance(other, IntervalCollection):
            res = collection_sub(
                _to_epoch_seconds(self.data.start),
                _to_epoch_seconds(self.data.stop),
                _to_epoch_seconds(other.data.start),
                _to_epoch_seconds(other.data.stop),
            )
            if len(res["start"]) == 0:
                return None
            data = pd.DataFrame({"start": res["start"], "stop": res["stop"]})
            data = data.eval(
                """start = @pd.to_datetime(start, unit="s", utc=True)
                   stop = @pd.to_datetime(stop, unit="s", utc=True)"""
            )
            return IntervalCollection(data)

        return NotImplemented

    def __and__(
        self, other: Interval | IntervalCollection
    ) -> None | IntervalCollection:
        """
        >>> i1 = Interval(1647861000, 1647861120)
        >>> i2 = Interval(1647861060, 1647861180)
        >>> i3 = Interval(1647861240, 1647861300)
        >>> i1 & i3
        >>> i1 & i2
        [... 11:11..., ... 11:12...]
        >>> (i1 + i2) & i2
        [[... 11:11..., ... 11:13...]]
        >>> (i1 + i2) & (i1 + i2 + i3)
        [[... 11:10..., ... 11:13...]]
        """
        return self.intersection(other)

    def __rand__(self, other: Interval) -> None | IntervalCollection:
        return self.intersection(other)

    def intersection(
        self, other: Interval | IntervalCollection
    ) -> None | IntervalCollection:
        """Returns the result of an intersection of intervals.

        :param other: the second interval or collection of intervals

        :return: may be `None` if the intersection is empty

        .. note::

            The binary operator `&` is equivalent to this method.

        """

        if isinstance(other, Interval):
            res = collection_andi(
                _to_epoch_seconds(self.data.start),
                _to_epoch_seconds(self.data.stop),
                int(other.start.timestamp()),
                int(other.stop.timestamp()),
            )
            if len(res["start"]) == 0:
                return None
            data = pd.DataFrame({"start": res["start"], "stop": res["stop"]})
            data = data.eval(
                """start = @pd.to_datetime(start, unit="s", utc=True)
                   stop = @pd.to_datetime(stop, unit="s", utc=True)"""
            )
            return IntervalCollection(data)

        if isinstance(other, IntervalCollection):
            res = collection_and(
                _to_epoch_seconds(self.data.start),
                _to_epoch_seconds(self.data.stop),
                _to_epoch_seconds(other.data.start),
                _to_epoch_seconds(other.data.stop),
            )
            if len(res["start"]) == 0:
                return None
            data = pd.DataFrame({"start": res["start"], "stop": res["stop"]})
            data = data.eval(
                """start = @pd.to_datetime(start, unit="s", utc=True)
                   stop = @pd.to_datetime(stop, unit="s", utc=True)"""
            )
            return IntervalCollection(data)

        return NotImplemented

    def total_duration(self) -> pd.Timedelta:
        """Returns the sum of durations of all intervals."""
        return self.data.eval("(stop - start).sum()")
