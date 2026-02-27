from __future__ import annotations

import os
from datetime import datetime, timezone
from pathlib import Path

import httpx
import pytest

from thrust.airports import (
    FaaArcgisAirportsSource,
    NasrAirportsSource,
)
from thrust.navpoints import (
    FaaArcgisNavpointsSource,
    NasrNavpointsSource,
)

ARCGIS_BASE = "https://opendata.arcgis.com/datasets"
NASR_BASE = "https://nfdc.faa.gov/webContent/28DaySub"

ARCGIS_FILES = {
    "faa_airports.json": "e747ab91a11045e8b3f8a3efd093d3b5_0",
    "faa_designated_points.json": "861043a88ff4486c97c3789e7dcdccc6_0",
    "faa_navaid_components.json": "c9254c171b6741d3a5e494860761443a_0",
}


def _cache_dir() -> Path:
    return Path(
        os.getenv(
            "FAA_TEST_DATA_DIR", str(Path.home() / ".cache" / "thrust-faa")
        )
    )


def _download(url: str, target: Path) -> None:
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists() and target.stat().st_size > 0:
        return

    with httpx.Client(follow_redirects=True, timeout=120.0) as client:
        response = client.get(url)
        response.raise_for_status()
        target.write_bytes(response.content)


def _resolve_nasr_url() -> str:
    explicit = os.getenv("FAA_NASR_URL")
    if explicit:
        return explicit

    airac = os.getenv("FAA_NASR_AIRAC")
    if airac:
        return f"{NASR_BASE}/28DaySubscription_Effective_{airac}.zip"

    now = datetime.now(timezone.utc)
    candidates: list[str] = []
    for year in [now.year + 1, now.year, now.year - 1]:
        yy = year % 100
        for cycle in range(13, 0, -1):
            candidates.append(f"{yy:02d}{cycle:02d}")

    for code in candidates:
        url = f"{NASR_BASE}/28DaySubscription_Effective_{code}.zip"
        try:
            with httpx.Client(follow_redirects=True, timeout=30.0) as client:
                head = client.head(url)
                if head.status_code == 405:
                    probe = client.get(url)
                    if probe.is_success:
                        return url
                elif head.is_success:
                    return url
        except httpx.HTTPError:
            continue

    raise RuntimeError(
        "Could not determine NASR cycle from FAA directory listing"
    )


def _ensure_arcgis_files() -> Path:
    root = _cache_dir() / "arcgis"
    for filename, dataset_id in ARCGIS_FILES.items():
        _download(f"{ARCGIS_BASE}/{dataset_id}.geojson", root / filename)
    return root


def _ensure_nasr_zip() -> Path:
    explicit_file = os.getenv("FAA_NASR_ZIP")
    if explicit_file:
        path = Path(explicit_file)
        if path.exists() and path.stat().st_size > 0:
            return path

    target = _cache_dir() / "nasr" / "nasr.zip"
    try:
        _download(_resolve_nasr_url(), target)
    except Exception as exc:  # pragma: no cover
        pytest.skip(f"Unable to fetch NASR archive in this environment: {exc}")
    return target


def test_faa_arcgis_airports_source_real_data() -> None:
    src = FaaArcgisAirportsSource(str(_ensure_arcgis_files()))

    rows = src.list_airports()
    assert len(rows) > 1000

    lax = src.resolve_airport("LAX")
    assert len(lax) >= 1
    assert any(row.latitude != 0.0 and row.longitude != 0.0 for row in lax)


def test_faa_arcgis_navpoints_source_real_data() -> None:
    src = FaaArcgisNavpointsSource(str(_ensure_arcgis_files()))

    fixes = src.resolve_point("DAG", "fix")
    assert len(fixes) >= 1

    navaids = src.resolve_point("LAX", "navaid")
    assert len(navaids) >= 1


def test_nasr_airports_and_navpoints_sources_real_data() -> None:
    nasr_zip = _ensure_nasr_zip()

    airport_src = NasrAirportsSource(str(nasr_zip))
    lax_airports = airport_src.resolve_airport("LAX")
    klax_airports = airport_src.resolve_airport("KLAX")
    assert len(lax_airports) + len(klax_airports) >= 1

    nav_src = NasrNavpointsSource(str(nasr_zip))
    fixes = nav_src.resolve_point("DAG", "fix")
    assert len(fixes) >= 1
    assert any(row.latitude != 0.0 and row.longitude != 0.0 for row in fixes)
