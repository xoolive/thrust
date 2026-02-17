from typing import Any

class Point:
    latitude: float
    longitude: float
    name: str | None

    def to_dict(self) -> dict[str, Any]: ...

class Segment:
    start: Point
    end: Point
    name: str | None

    def to_dict(self) -> dict[str, Any]: ...

class AiracDatabase:
    def __init__(self, path: str) -> None: ...
    def enrich_route(self, route: str) -> list[Segment]: ...
