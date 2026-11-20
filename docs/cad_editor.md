# CAD Object Editor

The CAD scene browses and edits the GLB models used by buildings, units, and
base-interior machine blocks. Select **Edit** on an object to edit its mesh and
face metadata. Changes can be saved to the selected asset or discarded to restore
the last saved version.

## View and edit controls

- Right-drag to orbit, middle-drag to pan, and use the mouse wheel to zoom.
- Choose vertex, edge, or face selection to select mesh elements.
- Choose **Move**, **Rotate**, or **Scale** on the transform toolbar. Drag the
  colored axis handles around the selection; the white center handle moves freely.
  Rotation uses colored rings, and scaling uses the cube handles. Release to
  commit one undoable edit; press `Escape` during a drag to cancel it.
- **Vertex Snap** is on by default for free Move drags. It snaps the selection
  center to visible unselected vertices and highlights the snap target. Toggle it
  in the view panel to move freely. Grid snap and spacing control primitive
  placement.
- **Value** opens a small, non-blocking XYZ input for exact translations,
  rotations in degrees, or scale factors. Press `Enter` or **Apply** to commit;
  close the input or press `Escape` to dismiss it.
- Use the primitive placement tools to add geometry; confirm placement with
  `Enter` and cancel with `Escape`.
- Use undo/redo while editing; save or discard to finish.

The interaction preview assembles a building from six neighboring hexes.
**Shuffle neighbors** changes the preview conditions. The preview only uses
building models (not routes or machine blocks).

## Radial Repeat and UCS origin

Radial Repeat groups describe geometry repeated around a six-sector hex. To set
one up, create a face group, configure it as **Radial Repeat**, and put the
conditional face groups beneath it. Conditional groups can match **Empty**,
**Same building**, or **Other building** neighbor conditions.

With a Radial Repeat group configured, the editor displays a triangular grid and
a colored UCS icon. The UCS origin is the center of the triangular grid and the
pivot used by the six-sector interaction preview and in-game route assembly. It
defaults to model origin `(0, 0, 0)`.

Use **Place UCS origin** or **Move UCS origin**, then click the construction
plane to set the origin. Press `Escape` to cancel placement. The placement
operation can be undone. When grid snapping is enabled, primitive positions use
the triangular grid centered on the UCS origin; other meshes continue to use
the square grid.

## Saved metadata

Mesh face names, groups, group programs, and the optional UCS origin are stored
in a JSON sidecar next to the model, with the `.cad.json` extension (for example,
`base.cad.json` next to `base.glb`). The GLB contains the mesh; the sidecar
contains editor metadata. Keep the sidecar with its model so face groups and
radial settings are available when the object is loaded again.

Older sidecars without a UCS origin remain valid and use the default model
origin.
