use ndarray_linalg::Inverse;
use numpy::ndarray::{s, Array, Array2, Array3};
use polars::prelude::SeriesOpsTime;
use polars::prelude::*;

type KalmanOutput = PolarsResult<(Array2<f64>, Array2<f64>, Array3<f64>, Array3<f64>)>;

pub fn kalman6d(df: DataFrame) -> KalmanOutput {
    let (n_rows, n_cols) = df.shape();
    let identity: Array2<f64> = Array2::eye(n_cols);
    let dt = 1.;
    let reject_sigma = 3.;

    // Initialize all results arrays
    let mut x_mes: Array2<f64> = Array2::zeros((n_rows, n_cols));

    for (j, name) in ["x", "y", "z", "dx", "dy", "dz"].into_iter().enumerate() {
        let col = df.column(name)?;
        for (i, value) in col.f64()?.iter().enumerate() {
            x_mes[(i, j)] = value.map_or(1e24, |v| v);
        }
    }

    let mut x_pre: Array2<f64> = Array2::zeros((n_rows, n_cols));
    let mut x_cor: Array2<f64> = Array2::zeros((n_rows, n_cols));
    let mut p_pre: Array3<f64> = Array3::zeros((n_rows, n_cols, n_cols));
    let mut p_cor: Array3<f64> = Array3::zeros((n_rows, n_cols, n_cols));

    // Definition of the R matrix

    let rolling_mean_params = RollingOptionsFixedWindow {
        window_size: 17,
        min_periods: 17, // by default equal to window_size
        weights: None,
        center: false,
        fn_params: None,
    };

    let df_x = df.column("x")?;
    let df_x_rolling_mean = df_x
        .as_materialized_series()
        .rolling_mean(rolling_mean_params.clone())?;
    let std_x = (df_x.as_materialized_series() - &df_x_rolling_mean)?
        .std(1)
        .unwrap_or(0.);

    let df_y = df.column("y")?;
    let df_y_rolling_mean = df_y
        .as_materialized_series()
        .rolling_mean(rolling_mean_params.clone())?;
    let std_y = (df_y.as_materialized_series() - &df_y_rolling_mean)?
        .std(1)
        .unwrap_or(0.);

    let df_z = df.column("z")?;
    let df_z_rolling_mean = df_z
        .as_materialized_series()
        .rolling_mean(rolling_mean_params.clone())?;
    let std_z = (df_z.as_materialized_series() - &df_z_rolling_mean)?
        .std(1)
        .unwrap_or(0.);

    let df_dx = df.column("dx")?;
    let df_dx_rolling_mean = df_dx
        .as_materialized_series()
        .rolling_mean(rolling_mean_params.clone())?;
    let std_dx = (df_dx.as_materialized_series() - &df_dx_rolling_mean)?
        .std(1)
        .unwrap_or(0.);

    let df_dy = df.column("dy")?;
    let df_dy_rolling_mean = df_dy
        .as_materialized_series()
        .rolling_mean(rolling_mean_params.clone())?;
    let std_dy = (df_dy.as_materialized_series() - &df_dy_rolling_mean)?
        .std(1)
        .unwrap_or(0.);

    let df_dz = df.column("dz")?;
    let df_dz_rolling_mean = df_dz
        .as_materialized_series()
        .rolling_mean(rolling_mean_params.clone())?;
    let std_dz = (df_dz.as_materialized_series() - &df_dz_rolling_mean)?
        .std(1)
        .unwrap_or(0.);

    let r_diag = Array::from_vec(vec![std_x, std_y, std_z, std_dx, std_dy, std_dz]);
    let r_matrix: Array2<f64> = Array2::from_diag(&r_diag) * Array2::from_diag(&r_diag);

    // Definition of the Q matrix

    let q_diag = Array::from_vec(vec![0.25, 0.25, 0.25, 1., 1., 1.]);
    let mut q_matrix: Array2<f64> = 1e-1 * Array2::from_diag(&q_diag) * &r_matrix;
    for i in 0..3 {
        q_matrix[(i, i + 3)] += 0.5 * q_matrix[(i + 3, i + 3)];
        q_matrix[(i + 3, i)] += 0.5 * q_matrix[(i + 3, i + 3)];
    }

    x_cor.slice_mut(s![0, ..]).assign(&x_mes.slice(s![0, ..]));
    p_pre.slice_mut(s![0, .., ..]).assign(&(1e5 * &identity));
    p_cor.slice_mut(s![0, .., ..]).assign(&(1e5 * &identity));

    // Model
    #[rustfmt::skip]
    let a_matrix: Array2<f64> = Array2::from_shape_vec(
        (6, 6),
        vec![
            1., 0., 0., dt, 0., 0.,
            0., 1., 0., 0., dt, 0.,
            0., 0., 1., 0., 0., dt,
            0., 0., 0., 1., 0., 0.,
            0., 0., 0., 0., 1., 0.,
            0., 0., 0., 0., 0., 1.,
        ],
    ).unwrap();

    for i in 1..n_rows {
        x_pre
            .slice_mut(s![i, ..])
            .assign(&a_matrix.dot(&x_cor.slice(s![i - 1, ..])));

        p_pre
            .slice_mut(s![i, .., ..])
            .assign(&(a_matrix.dot(&p_cor.slice(s![i - 1, .., ..])).dot(&a_matrix.t()) + &q_matrix));

        let mut h_matrix = identity.clone();

        // Innovation
        let mut nu = &x_mes.slice(s![i, ..]) - &x_pre.slice(s![i, ..]);
        let s_matrix: Array2<f64> = h_matrix.dot(&p_pre.slice(s![i, .., ..])).dot(&h_matrix.t()) + &r_matrix;

        // Explicitly annotate type to help compiler resolve trait
        let mut s_inverse: Array2<f64> = s_matrix.inv().unwrap();
        if nu.t().dot(&s_inverse).dot(&nu) > reject_sigma * reject_sigma * n_cols as f64 {
            let x = s_inverse.dot(&nu) * &nu;
            for (j, &val) in x.iter().enumerate() {
                if val > reject_sigma.powi(2) {
                    x_mes[(i, j)] = x_pre[(i, j)];
                    nu[j] = 0.;
                    h_matrix[(j, j)] = 0.;
                }
            }
            // Recompute s_matrix since we need it again
            let s_matrix: Array2<f64> = h_matrix.dot(&p_pre.slice(s![i, .., ..])).dot(&h_matrix.t()) + &r_matrix;
            s_inverse = s_matrix.inv().unwrap();
        }

        let k_gain = p_pre.slice(s![i, .., ..]).dot(&h_matrix.t()).dot(&s_inverse);

        x_cor
            .slice_mut(s![i, ..])
            .assign(&(&x_pre.slice(s![i, ..]) + k_gain.dot(&nu)));

        let imkh = &identity - k_gain.dot(&h_matrix);
        p_cor
            .slice_mut(s![i, .., ..])
            .assign(&(imkh.dot(&p_pre.slice(s![i, .., ..])).dot(&imkh.t()) + k_gain.dot(&r_matrix).dot(&k_gain.t())))
    }

    Ok((x_pre, x_cor, p_pre, p_cor))
}
