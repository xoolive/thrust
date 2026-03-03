import { loadEurocontrolInputs, loadWasmModule } from "./helpers";

type Resolver = {
  airports: () => unknown;
  resolve_airport: (code: string) => unknown;
  resolve_fix: (code: string) => unknown;
  resolve_navaid: (code: string) => unknown;
  resolve_airway: (name: string) => unknown;
};

function assertCoreEurocontrolEntities(
  resolver: Resolver,
  label: string,
  options: {
    checkAirportNames?: boolean;
    checkRouteClass?: boolean;
  } = {},
): void {
  const airports = resolver.airports() as Array<{ code?: string }>;
  const airportCodes = new Set(
    airports.map((a) => String(a.code || "").toUpperCase()),
  );
  for (const code of ["EHAM", "LSZH", "LFCL", "LFCX"]) {
    if (!airportCodes.has(code)) {
      throw new Error(`[${label}] Missing Eurocontrol airport ${code}`);
    }
  }

  if (options.checkAirportNames) {
    const eham = resolver.resolve_airport("EHAM") as { name?: string } | null;
    const ehamName = String(eham?.name || "").toUpperCase();
    if (
      !eham ||
      (!ehamName.includes("SCHIPHOL") && !ehamName.includes("AMSTERDAM"))
    ) {
      throw new Error(
        `[${label}] Unexpected EHAM airport name: ${String(eham?.name || "")}`,
      );
    }

    const lszh = resolver.resolve_airport("LSZH") as { name?: string } | null;
    if (
      !lszh ||
      !String(lszh.name || "")
        .toUpperCase()
        .includes("ZURICH")
    ) {
      throw new Error(
        `[${label}] Unexpected LSZH airport name: ${String(lszh?.name || "")}`,
      );
    }
  }

  const lfbo = resolver.resolve_airport("LFBO") as {
    latitude?: number;
    longitude?: number;
  } | null;
  const lat = Number(lfbo?.latitude);
  const lon = Number(lfbo?.longitude);
  if (!lfbo || !Number.isFinite(lat) || !Number.isFinite(lon)) {
    throw new Error(`[${label}] Missing LFBO coordinates`);
  }
  if (Math.abs(lat - 43.635) > 0.05 || Math.abs(lon - 1.368) > 0.05) {
    throw new Error(`[${label}] Unexpected LFBO coordinates: ${lat}, ${lon}`);
  }

  const narak = resolver.resolve_fix("NARAK") as { code?: string } | null;
  if (!narak || String(narak.code || "").toUpperCase() !== "NARAK") {
    throw new Error(`[${label}] Failed to resolve Eurocontrol fix NARAK`);
  }

  const narakAsNavaid = resolver.resolve_navaid("NARAK") as { code?: string } | null;
  if (!narakAsNavaid || String(narakAsNavaid.code || "").toUpperCase() !== "NARAK") {
    throw new Error(`[${label}] Failed to resolve Eurocontrol navaid NARAK`);
  }

  const gai = resolver.resolve_navaid("GAI") as {
    code?: string;
    description?: string;
  } | null;
  if (!gai || String(gai.code || "").toUpperCase() !== "GAI") {
    throw new Error(`[${label}] Failed to resolve Eurocontrol navaid GAI`);
  }
  if (
    !String(gai.description || "")
      .toUpperCase()
      .includes("GAILLAC")
  ) {
    throw new Error(
      `[${label}] Unexpected GAI navaid description: ${String(gai.description || "")}`,
    );
  }

  const tou = resolver.resolve_navaid("TOU") as {
    code?: string;
    description?: string;
  } | null;
  if (!tou || String(tou.code || "").toUpperCase() !== "TOU") {
    throw new Error(`[${label}] Failed to resolve Eurocontrol navaid TOU`);
  }
  if (
    !String(tou.description || "")
      .toUpperCase()
      .includes("TOULOUSE")
  ) {
    throw new Error(
      `[${label}] Unexpected TOU navaid description: ${String(tou.description || "")}`,
    );
  }

  const um605 = resolver.resolve_airway("UM605") as {
    name?: string;
    points?: unknown[];
  } | null;
  if (!um605 || String(um605.name || "").toUpperCase() !== "UM605") {
    throw new Error(`[${label}] Failed to resolve Eurocontrol airway UM605`);
  }
  if (!Array.isArray(um605.points) || um605.points.length < 2) {
    throw new Error(`[${label}] UM605 should contain at least 2 points`);
  }

  if (options.checkRouteClass) {
    const routeClass = String(
      (um605 as { route_class?: string }).route_class || "",
    ).toUpperCase();
    if (routeClass !== "AR") {
      throw new Error(`[${label}] Unexpected UM605 route_class: ${routeClass}`);
    }
  }
}

const inputs = loadEurocontrolInputs();
if (!inputs) {
  throw new Error(
    "THRUST_AIXM_PATH/THRUST_DDR_PATH data is required for eurocontrol.spec.ts",
  );
} else {
  const wasmModule = await loadWasmModule();
  const aixmResolver = new wasmModule.EurocontrolResolver(inputs.aixmFolder);
  const ddrResolver = inputs.ddrArchive
    ? wasmModule.EurocontrolResolver.fromDdrArchive(inputs.ddrArchive)
    : wasmModule.EurocontrolResolver.fromDdrFolder(inputs.ddrFolder!);

  // DDR .arp inputs do not carry airport names, so name assertions are AIXM-only.
  assertCoreEurocontrolEntities(aixmResolver, "AIXM", {
    checkAirportNames: true,
  });
  assertCoreEurocontrolEntities(ddrResolver, "DDR", {
    checkRouteClass: true,
  });

  console.log("eurocontrol.spec.ts passed");
}
