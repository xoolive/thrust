from typing import TypedDict

import numpy as np
import numpy.typing as npt
import polars as pl

class KalmanResult(TypedDict):
    x_cor: list[npt.NDArray[np.float64]]

def kalman6d_rs(data: pl.DataFrame) -> KalmanResult: ...
