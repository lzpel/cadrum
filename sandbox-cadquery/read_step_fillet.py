"""issue #129 検証: NG_multicolor.step / OK_singlecolor.step を CadQuery で読み、
全エッジに fillet をかけて挙動を比較する。

CadQuery (= OCCT) が同じ STEP に対して solid 0 個になるか、何らかの workaround を
内蔵していて 1 solid として扱えるかを確認する。"""

from pathlib import Path

import cadquery as cq


def report(label: str, step_path: Path, fillet_radius: float = 0.3) -> None:
    print(f"\n=== {label}: {step_path.name} ===")
    try:
        imported = cq.importers.importStep(str(step_path))
    except Exception as e:
        print(f"  importStep failed: {e}")
        return

    # importStep returns a Workplane wrapping the imported shape(s)
    shapes = imported.vals()
    print(f"  Workplane.vals() length: {len(shapes)}")
    for i, sh in enumerate(shapes):
        print(f"    [{i}] type={sh.geomType() if hasattr(sh, 'geomType') else type(sh).__name__} "
              f"shapeType={sh.ShapeType() if hasattr(sh, 'ShapeType') else 'n/a'}")

    # Inspect underlying compound: count solids / shells / faces / edges
    compound = imported.val()
    print(f"  ShapeType: {compound.ShapeType()}")
    print(f"  Solids:  {len(compound.Solids())}")
    print(f"  Shells:  {len(compound.Shells())}")
    print(f"  Faces:   {len(compound.Faces())}")
    print(f"  Edges:   {len(compound.Edges())}")

    edges = imported.edges()
    print(f"  Workplane.edges().vals(): {len(edges.vals())}")

    # Try fillet on all edges
    print(f"  Trying fillet(radius={fillet_radius}) on all edges...")
    try:
        filleted = imported.edges().fillet(fillet_radius)
        result = filleted.val()
        print(f"  [OK] fillet  Solids: {len(result.Solids())}  "
              f"Faces: {len(result.Faces())}  Edges: {len(result.Edges())}")
        out = step_path.with_name(step_path.stem + "_filleted.step")
        cq.exporters.export(filleted, str(out))
        print(f"  wrote: {out.name}")
    except Exception as e:
        print(f"  [FAIL] fillet failed: {type(e).__name__}: {e}")


if __name__ == "__main__":
    here = Path(__file__).parent
    report("OK (single-color)",  here / "OK_singlecolor.step")
    report("NG (multi-color)",   here / "NG_multicolor.step")
