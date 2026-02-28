from __future__ import annotations

import os
from pathlib import Path

from thrust.airports import AixmAirportsSource, DdrAirportsSource
from thrust.airspaces import AixmAirspacesSource, DdrAirspacesSource
from thrust.airways import AixmAirwaysSource, DdrAirwaysSource
from thrust.navpoints import AixmNavpointsSource, DdrNavpointsSource


def _aixm_path() -> Path | None:
    value = os.getenv("THRUST_AIXM_PATH")
    return Path(value) if value else None


def _ddr_path() -> Path | None:
    value = os.getenv("THRUST_DDR_PATH")
    return Path(value) if value else None


def test_aixm_sources_parse_when_folder_available() -> None:
    path = _aixm_path()
    if path is None or not path.exists():
        return

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

    assert len(navpoint_src.resolve_point("NARAK", "fix")) >= 1
    gai = navpoint_src.resolve_point("GAI", "navaid")
    tou = navpoint_src.resolve_point("TOU", "navaid")
    assert len(gai) >= 1
    assert len(tou) >= 1
    assert all(str(row.name or "").strip() for row in gai)
    assert all(str(row.name or "").strip() for row in tou)
    assert len(AixmAirwaysSource(path).resolve_airway("UM605")) >= 1


def test_ddr_sources_parse_when_folder_available() -> None:
    path = _ddr_path()
    if path is None or not path.exists():
        return

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

    assert len(navpoint_src.resolve_point("NARAK", "fix")) >= 1
    assert len(navpoint_src.resolve_point("GAI", "navaid")) >= 1
    assert len(navpoint_src.resolve_point("TOU", "navaid")) >= 1
    assert len(DdrAirwaysSource(path).resolve_airway("UM605")) >= 1
