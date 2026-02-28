from __future__ import annotations

import os
from pathlib import Path

from thrust.airports import AixmAirportsSource, DdrAirportsSource
from thrust.airspaces import AixmAirspacesSource, DdrAirspacesSource
from thrust.airways import AixmAirwaysSource, DdrAirwaysSource
from thrust.navpoints import AixmNavpointsSource, DdrNavpointsSource


def _aixm_path() -> Path:
    value = os.getenv("THRUST_AIXM_PATH")
    assert value is not None, (
        "THRUST_AIXM_PATH must be set for Eurocontrol tests"
    )
    path = Path(value).expanduser()
    assert path.exists(), f"THRUST_AIXM_PATH does not exist: {path}"
    return path


def _ddr_path() -> Path:
    value = os.getenv("THRUST_DDR_PATH")
    assert value is not None, (
        "THRUST_DDR_PATH must be set for Eurocontrol tests"
    )
    path = Path(value).expanduser()
    assert path.exists(), f"THRUST_DDR_PATH does not exist: {path}"
    return path


def test_aixm_sources_parse_when_folder_available() -> None:
    path = _aixm_path()

    airports = AixmAirportsSource(path).list_airports()
    navpoint_src = AixmNavpointsSource(path)
    fixes = navpoint_src.list_points("fix")
    navaids = navpoint_src.list_points("navaid")
    airways = AixmAirwaysSource(path).list_airways()
    airspaces = AixmAirspacesSource(path).list_airspaces()

    assert len(airports) > 1000
    assert len(fixes) + len(navaids) > 1000
    assert isinstance(airways, list)
    assert len(airspaces) > 100

    airport_codes = {row.code.upper() for row in airports}
    for code in ["EHAM", "LSZH", "LFCL", "LFCX"]:
        assert code in airport_codes

    eham = AixmAirportsSource(path).resolve_airport("EHAM")
    assert len(eham) >= 1
    assert any("SCHIPHOL" in str(row.name or "").upper() for row in eham)

    lszh = AixmAirportsSource(path).resolve_airport("LSZH")
    assert len(lszh) >= 1
    assert any("ZURICH" in str(row.name or "").upper() for row in lszh)

    # Depending on DDR cycle/source typing,
    # NARAK can be exposed as FIX or NAVAID.
    narak_fix = navpoint_src.resolve_point("NARAK", "fix")
    narak_navaid = navpoint_src.resolve_point("NARAK", "navaid")
    assert len(narak_fix) + len(narak_navaid) >= 1
    gai = navpoint_src.resolve_point("GAI", "navaid")
    tou = navpoint_src.resolve_point("TOU", "navaid")
    assert len(gai) >= 1
    assert len(tou) >= 1
    assert all(str(row.name or "").strip() for row in gai)
    assert all(str(row.name or "").strip() for row in tou)
    assert len(AixmAirwaysSource(path).resolve_airway("UM605")) >= 1


def test_ddr_sources_parse_when_folder_available() -> None:
    path = _ddr_path()

    airports = DdrAirportsSource(path).list_airports()
    navpoint_src = DdrNavpointsSource(path)
    fixes = navpoint_src.list_points("fix")
    navaids = navpoint_src.list_points("navaid")
    airways = DdrAirwaysSource(path).list_airways()
    airspaces = DdrAirspacesSource(path).list_airspaces()

    assert len(airports) > 100
    assert len(fixes) + len(navaids) > 1000
    assert len(airways) > 100
    assert len(airspaces) > 100

    airport_codes = {row.code.upper() for row in airports}
    for code in ["EHAM", "LSZH", "LFCL", "LFCX"]:
        assert code in airport_codes

    narak_fix = navpoint_src.resolve_point("NARAK", "fix")
    narak_navaid = navpoint_src.resolve_point("NARAK", "navaid")
    assert len(narak_fix) + len(narak_navaid) >= 1
    assert len(navpoint_src.resolve_point("GAI", "navaid")) >= 1
    assert len(navpoint_src.resolve_point("TOU", "navaid")) >= 1
    assert len(DdrAirwaysSource(path).resolve_airway("UM605")) >= 1
