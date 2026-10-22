/**
 * The v3 plan parser (`parseImportPlan`) against the server's own v3
 * fixtures: what it parses serialises back to the same JSON, and it refuses
 * what serde refuses.
 */
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { parseImportPlan } from "../src/api/import-plan-tree-read";
import { isRecord, isUnknownArray } from "../src/api/parse";

const HERE = dirname(fileURLToPath(import.meta.url));
const RUST_V3_PLANS = resolve(
  HERE,
  "../../../crates/omeka-api/src/import/fixtures/v3",
);
const FIXTURES = [
  "bhc006.json",
  "archive_as_found.json",
  "one_series.json",
];

const rustPlan = (name: string): Record<string, unknown> => {
  const value: unknown = JSON.parse(
    readFileSync(resolve(RUST_V3_PLANS, name), "utf8"),
  );
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${name} is a plan object`);
  }
  return { ...value };
};

const roundTrip = (raw: unknown): unknown => {
  const back: unknown = JSON.parse(JSON.stringify(parseImportPlan(raw)));
  return back;
};

/** The BHC006 plan with its first top level replaced. */
const withFirstLevel = (
  change: (level: Record<string, unknown>) => Record<string, unknown>,
): unknown => {
  const plan = rustPlan("bhc006.json");
  const structure: unknown = plan["structure"];
  const levels: unknown = isRecord(structure) ? structure["levels"] : undefined;
  const [first, ...rest] = isUnknownArray(levels) ? levels : [];
  if (!isRecord(structure) || !isRecord(first)) {
    throw new Error("bhc006 has a top level");
  }
  return {
    ...plan,
    structure: { ...structure, levels: [change(first), ...rest] },
  };
};

describe("the v3 plan", () => {
  it.each(FIXTURES)(
    "parses each server v3 fixture and writes it back unchanged (%s)",
    (name) => {
      const raw = rustPlan(name);
      expect(roundTrip(raw)).toEqual(raw);
    },
  );

  it("writes back the set sources the fixtures do not use", () => {
    const each = {
      sources: [{ from: "folder", level: 1 }],
      combine: "first",
      map: { Misc: "" },
      names: "id",
    };
    const plan = withFirstLevel((level) => ({
      ...level,
      set: { title: "Citizen Curators", description: "Gifts, 2018" },
    }));
    const raw =
      isRecord(plan) && isRecord(plan["structure"])
        ? {
            ...plan,
            structure: {
              ...plan["structure"],
              freeSets: [{ id: 42 }, { each }],
            },
          }
        : plan;
    expect(roundTrip(raw)).toEqual(raw);
  });

  it("writes back a join by the last places of names, and leaves it out when off", () => {
    const raw: unknown = JSON.parse(JSON.stringify(rustPlan("bhc006.json")));
    const levels = isRecord(raw) && isRecord(raw["structure"]) ? raw["structure"]["levels"] : undefined;
    const project = isUnknownArray(levels) ? levels[0] : undefined;
    const series = isRecord(project) && isUnknownArray(project["levels"]) ? project["levels"][2] : undefined;
    const lists = isRecord(series) ? series["lists"] : undefined;
    const list = isUnknownArray(lists) ? lists[0] : undefined;
    const join = isRecord(list) ? list["join"] : undefined;
    const folds = isRecord(join) ? join["normalize"] : undefined;
    if (!isRecord(join) || !isRecord(folds)) {
      throw new Error("bhc006 joins its D list");
    }
    expect(roundTrip(raw)).toEqual(raw);
    expect(JSON.stringify(roundTrip(raw))).not.toContain("lastPlaces");
    join["normalize"] = { ...folds, lastPlaces: 2 };
    expect(roundTrip(raw)).toEqual(raw);
    join["normalize"] = { ...folds, lastPlaces: "2" };
    expect(() => parseImportPlan(raw)).toThrow("lastPlaces");
  });

  it("writes back the paths a structure leaves out, and leaves the member out when empty", () => {
    const plan = rustPlan("bhc006.json");
    const structure = plan["structure"];
    if (!isRecord(structure)) {
      throw new Error("bhc006 has a structure");
    }
    expect(JSON.stringify(roundTrip(plan))).not.toContain("outside");
    const raw = {
      ...plan,
      structure: { ...structure, outside: [{ source: "s1", folder: "Artifacts/~lock.docx" }] },
    };
    expect(roundTrip(raw)).toEqual(raw);
    expect(() =>
      parseImportPlan({ ...plan, structure: { ...structure, outside: [{ source: "s1" }] } }),
    ).toThrow("outside");
  });

  it("refuses a member serde does not know", () => {
    expect(() =>
      parseImportPlan({
        ...rustPlan("bhc006.json"),
        grouping: { pattern: null, caseFold: false },
      }),
    ).toThrow("$.grouping: expected a member of this shape");
    expect(() =>
      parseImportPlan(
        withFirstLevel((level) => ({ ...level, scope: "all" })),
      ),
    ).toThrow("$.structure.levels[0].scope: expected a member of this shape");
  });

  it("refuses a level kind serde does not know", () => {
    expect(() =>
      parseImportPlan(
        withFirstLevel((level) => ({ ...level, kind: "folder" })),
      ),
    ).toThrow("$.structure.levels[0].kind: expected one of set, series");
  });

  it("refuses a set source with both an id and a title", () => {
    expect(() =>
      parseImportPlan(
        withFirstLevel((level) => ({
          ...level,
          set: { id: 42, title: "Citizen Curators", description: null },
        })),
      ),
    ).toThrow(
      "$.structure.levels[0].set.title: expected a member of this shape",
    );
  });

  it("refuses a version 2 plan", () => {
    expect(() =>
      parseImportPlan({ ...rustPlan("bhc006.json"), version: 2 }),
    ).toThrow("$.version: expected version 3");
  });
});
