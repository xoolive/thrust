from __future__ import annotations

import os
from datetime import datetime, timezone
from pathlib import Path

import httpx
import stamina

from thrust.airac import effective_date_from_airac_code
from thrust.airports import (
    FaaArcgisAirportsSource,
    NasrAirportsSource,
)
from thrust.airways import FaaArcgisAirwaysSource, NasrAirwaysSource
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
ATS_ROUTES_URL = (
    "https://hub.arcgis.com/api/v3/datasets/"
    "acf64966af5f48a1a40fdbcb31238ba7_0/downloads/data"
    "?format=geojson&spatialRefId=4326&where=IDENT%3D%27J48%27"
)


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

    @stamina.retry(on=httpx.HTTPError, attempts=3)
    def _fetch_content() -> bytes:
        with httpx.Client(follow_redirects=True, timeout=120.0) as client:
            response = client.get(url)
            response.raise_for_status()
            return response.content

    target.write_bytes(_fetch_content())


def _resolve_nasr_url() -> str:
    explicit = os.getenv("FAA_NASR_URL")
    if explicit:
        return explicit

    airac = os.getenv("FAA_NASR_AIRAC")
    if airac:
        preferred = (
            f"{NASR_BASE}/28DaySubscription_Effective_"
            f"{effective_date_from_airac_code(airac)}.zip"
        )
        try:
            with httpx.Client(follow_redirects=True, timeout=30.0) as client:
                head = client.head(preferred)
                if head.status_code == 405:
                    probe = client.get(preferred)
                    if probe.is_success:
                        return preferred
                elif head.is_success:
                    return preferred
                else:
                    probe = client.get(preferred)
                    if probe.is_success:
                        return preferred
        except httpx.HTTPError:
            pass

    now = datetime.now(timezone.utc)
    candidates: list[str] = []
    for year in [now.year + 1, now.year, now.year - 1]:
        yy = year % 100
        for cycle in range(13, 0, -1):
            candidates.append(f"{yy:02d}{cycle:02d}")

    for code in candidates:
        try:
            effective = effective_date_from_airac_code(code)
        except ValueError:
            continue
        url = f"{NASR_BASE}/28DaySubscription_Effective_{effective}.zip"
        try:
            with httpx.Client(follow_redirects=True, timeout=30.0) as client:
                head = client.head(url)
                if head.status_code == 405:
                    probe = client.get(url)
                    if probe.is_success:
                        return url
                elif head.is_success:
                    return url
                else:
                    probe = client.get(url)
                    if probe.is_success:
                        return url
        except httpx.HTTPError:
            continue

    raise RuntimeError("Could not find a reachable NASR subscription URL")


def _nasr_filename_from_url(url: str) -> str:
    return url.rstrip("/").split("/")[-1]


def _ensure_arcgis_files() -> Path:
    root = _cache_dir() / "arcgis"
    for filename, dataset_id in ARCGIS_FILES.items():
        _download(f"{ARCGIS_BASE}/{dataset_id}.geojson", root / filename)
    _download(ATS_ROUTES_URL, root / "faa_ats_routes.json")
    return root


def _ensure_nasr_zip() -> Path:
    explicit_file = os.getenv("FAA_NASR_ZIP")
    if explicit_file:
        path = Path(explicit_file)
        if path.exists() and path.stat().st_size > 0:
            return path

    nasr_root = _cache_dir() / "nasr"
    if nasr_root.exists():
        named = sorted(
            p
            for p in nasr_root.glob("28DaySubscription_Effective_*.zip")
            if p.is_file() and p.stat().st_size > 0
        )
        if named:
            return named[-1]

    configured_airac = os.getenv("FAA_NASR_AIRAC")
    if configured_airac:
        try:
            effective = effective_date_from_airac_code(configured_airac)
            named = nasr_root / f"28DaySubscription_Effective_{effective}.zip"
            if named.exists() and named.stat().st_size > 0:
                return named
        except ValueError:
            pass

    url = _resolve_nasr_url()
    target = nasr_root / _nasr_filename_from_url(url)
    if target.exists() and target.stat().st_size > 0:
        return target
    _download(url, target)
    return target


def test_faa_arcgis_airports_source_real_data() -> None:
    src = FaaArcgisAirportsSource(_ensure_arcgis_files())

    rows = src.list_airports()
    assert len(rows) > 1000

    codes = {row.code.upper() for row in rows}
    icaos = {str(row.icao or "").upper() for row in rows}
    for code in ["KLAX", "KATL", "KJFK", "KORD"]:
        assert code in codes or code in icaos

    assert any(
        code in codes or code in icaos
        for code in ["CYVR", "CYUL", "YVR", "YUL"]
    )

    lax = src.resolve_airport("LAX")
    assert len(lax) >= 1
    assert any(row.latitude != 0.0 and row.longitude != 0.0 for row in lax)
    assert any("LOS ANGELES" in str(row.name or "").upper() for row in lax)


def test_faa_arcgis_navpoints_source_real_data() -> None:
    src = FaaArcgisNavpointsSource(_ensure_arcgis_files())

    fixes = src.resolve_point("BASYE", "fix")
    assert len(fixes) >= 1

    navaids = src.resolve_point("BAF", "navaid")
    assert len(navaids) >= 1
    assert any("BARNES" in str(row.name or "").upper() for row in navaids)

    airways = FaaArcgisAirwaysSource(_ensure_arcgis_files()).resolve_airway(
        "J48"
    )
    assert len(airways) >= 1


def test_nasr_airports_and_navpoints_sources_real_data() -> None:
    nasr_zip = _ensure_nasr_zip()

    airport_src = NasrAirportsSource(nasr_zip)
    all_airports = airport_src.list_airports()
    codes = {row.code.upper() for row in all_airports}
    icaos = {str(row.icao or "").upper() for row in all_airports}
    for code in ["KLAX", "KATL", "KJFK", "KORD"]:
        assert code in codes or code in icaos
    lax = airport_src.resolve_airport("KLAX")
    assert len(lax) >= 1
    assert any("LOS ANGELES" in str(row.name or "").upper() for row in lax)

    nav_src = NasrNavpointsSource(nasr_zip)
    fixes = nav_src.resolve_point("BASYE", "fix")
    assert len(fixes) >= 1
    assert any(row.latitude != 0.0 and row.longitude != 0.0 for row in fixes)

    bafi = nav_src.resolve_point("BAF", "navaid")
    assert len(bafi) >= 1
    assert any("BARNES" in str(row.name or "").upper() for row in bafi)

    airways = NasrAirwaysSource(nasr_zip).resolve_airway("J48")
    assert len(airways) >= 1
