# type: ignore
# %%
import timeit

import pandas as pd
from cartes.crs import EuroPP
from traffic.algorithms.filters import ProcessXYZFilterBase
from traffic.algorithms.filters.kalman import KalmanFilter6D
from traffic.data.samples import noisy_landing

from thrust.kalman import kalman6d


class KalmanFilter6DRust(ProcessXYZFilterBase):
    def apply(self, data: pd.DataFrame) -> pd.DataFrame:
        df = self.preprocess(data)
        res = kalman6d(df)

        filtered = pd.DataFrame(
            res["x_cor"],
            columns=["x", "y", "z", "dx", "dy", "dz"],
        )

        return data.assign(**self.postprocess(filtered))


def test_correctness() -> None:
    noisy = noisy_landing.compute_xy(EuroPP())
    rs_kalman = KalmanFilter6DRust()
    kalman = noisy.filter(rs_kalman)

    assert kalman.altitude_max < 15000


def test_performance() -> None:
    noisy = noisy_landing.compute_xy(EuroPP())
    py_kalman = KalmanFilter6D()
    rs_kalman = KalmanFilter6DRust()

    timer = timeit.Timer("noisy.filter(py_kalman)", globals=locals())
    number, _ = (timer).autorange()
    best_time_py = min(timer.repeat(repeat=7, number=number)) / number

    timer = timeit.Timer("noisy.filter(rs_kalman)", globals=locals())
    number, _ = (timer).autorange()
    best_time_rs = min(timer.repeat(repeat=7, number=number)) / number

    print(f"Python implementation: {best_time_py:.3g} seconds")
    print(f"Rust implementation: {best_time_rs:.3g} seconds")
    print(f"Speed-up: {best_time_py / best_time_rs:.1f}x")

    # Speed-up should be 10x but let's be safe
    assert best_time_py / best_time_rs > 5


def test_visualize() -> None:
    # %%
    import altair as alt

    rs_kalman = KalmanFilter6DRust()
    noisy = noisy_landing.compute_xy(EuroPP())
    kalman = noisy.filter(rs_kalman)

    chart = (
        alt.layer(
            noisy.assign(type="Noisy data").chart(),
            kalman.assign(type="Kalman filter (Rust)").chart(),
        )
        .encode(
            alt.X("utchoursminutes(timestamp):T")
            .title(None)
            .axis(format="%H:%M"),
            alt.Y("altitude:Q")
            .title("altitude (in ft)")
            .axis(
                titleAnchor="end", titleAngle=0, titleAlign="left", titleY=-15
            ),
            alt.Color("type:N").legend(title=None, orient="bottom"),
        )
        .properties(width=400, height=200)
        .configure_axis(
            labelFontSize=14,
            titleFontSize=16,
            labelFont="Roboto Condensed",
            titleFont="Roboto Condensed",
        )
        .configure_legend(labelFontSize=14, labelFont="Roboto Condensed")
    )
    chart


# %%
