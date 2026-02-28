# Explicitly import and expose the submodules
from typing import TypedDict

import numpy as np
import numpy.typing as npt

class IntervalCollectionDict(TypedDict):
    start: npt.NDArray[np.int64]
    stop: npt.NDArray[np.int64]

class IntervalDict(TypedDict):
    start: int
    stop: int

def interval_and(
    start1: int, stop1: int, start2: int, stop2: int
) -> IntervalDict: ...
def interval_sub(
    start1: int, stop1: int, start2: int, stop2: int
) -> IntervalCollectionDict: ...
def collection_and(
    start1: npt.NDArray[np.int64],
    stop1: npt.NDArray[np.int64],
    start2: npt.NDArray[np.int64],
    stop2: npt.NDArray[np.int64],
) -> IntervalCollectionDict: ...
def collection_andi(
    start1: npt.NDArray[np.int64],
    stop1: npt.NDArray[np.int64],
    start2: int,
    stop2: int,
) -> IntervalCollectionDict: ...
def interval_add(
    start1: int, stop1: int, start2: int, stop2: int
) -> IntervalCollectionDict: ...
def collection_add(
    start1: npt.NDArray[np.int64],
    stop1: npt.NDArray[np.int64],
    start2: npt.NDArray[np.int64],
    stop2: npt.NDArray[np.int64],
) -> IntervalCollectionDict: ...
def collection_addi(
    start1: npt.NDArray[np.int64],
    stop1: npt.NDArray[np.int64],
    start2: int,
    stop2: int,
) -> IntervalCollectionDict: ...
def collection_sub(
    start1: npt.NDArray[np.int64],
    stop1: npt.NDArray[np.int64],
    start2: npt.NDArray[np.int64],
    stop2: npt.NDArray[np.int64],
) -> IntervalCollectionDict: ...
def collection_subi(
    start1: npt.NDArray[np.int64],
    stop1: npt.NDArray[np.int64],
    start2: int,
    stop2: int,
) -> IntervalCollectionDict: ...
