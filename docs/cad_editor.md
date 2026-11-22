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
- **Object Snaps...** opens independent toggles for center, tangent, quadrant,
  intersection, endpoint, midpoint, and perpendicular targets. They are enabled
  by default and apply to free Move drags. Targets must be visible; the active
  snap type is shown by the cursor. Center, tangent, and quadrant targets are
  offered only for recognized circular edge loops. The same popover contains
  **Polar** and **Grid Snap** toggles.
- **Polar** constrains free Move drags to the nearest 15-degree direction and
  is off by default. **Grid Snap** rounds the moved selection center to the
  configured grid spacing. These toggles affect the current Move workflow;
  Grid Snap also controls primitive placement. Grid snapping is off by default.
- **Value** opens a small, non-blocking XYZ input for exact translations,
  rotations in degrees, or scale factors. Press `Enter` or **Apply** to commit;
  close the input or press `Escape` to dismiss it.
- Use the primitive placement tools to add geometry; confirm placement with
  `Enter` and cancel with `Escape`.
- **Trim** starts by selecting target face(s); `Shift` toggles faces in the
  selection. Press `Enter` to choose cutter face(s), then select the mesh faces
  whose intersections should split the targets. Press `Enter` to split; both
  sides are retained. Alternatively, use **Draw Path** to draw a connected,
  boundary-to-boundary multiline on one target face. Press `Enter` to finish
  the path, then press it again to split. The path is temporary and is not
  saved. Trim currently splits triangular faces at face-face intersections;
  coplanar cutter faces do not define a cut line.
  Press `Escape` to cancel the active Trim operation.
- **Mirror** uses three clicks to define a plane. The reflected copy previews in
  the viewport; press `Enter` to keep it alongside the original, or `Escape` to
  cancel. `Backspace` removes the last plane point.
- **Fillet** rounds selected mesh edge(s). Select edges, choose **Fillet**, enter
  a radius in the viewport value panel, and click **Apply** or press `Enter`.
  The current mesh operation supports manifold creases and rejects invalid or
  overlapping edge sets.
- **Offset** copies selected faces along their normals, leaving the source
  unchanged. Select faces, choose **Offset**, enter a signed distance, then
  click **Apply** or press `Enter`.
- Mirror, Fillet, Offset, and Trim are undoable single operations. Their mesh
  previews stay in the viewport; errors such as unsupported topology or
  invalid values leave the original mesh unchanged.
- Use undo/redo while editing; save or discard to finish.
- Use the **Face Paint** panel in the right sidebar. Select faces or a face
  group in Face selection mode, then click a palette color to paint the current
  selection. Double-click the hex field to enter a precise 6-digit RGB or
  8-digit RGBA color, then press `Enter` to apply it.
- Painting is available only for selected faces and face groups, not vertex or
  edge selections. Paint changes use vertex colors stored in the GLB. Face
  names, groups, and other CAD metadata remain in the `.cad.json` sidecar. Save
  and discard work the same way as other mesh edits.

The interaction preview assembles the selected building and six surrounding
cells. Radial neighbors use the selected cell and the two adjacent occupied
ring cells to choose their six sector variants; their other sectors are Empty.
The **Same building** condition reuses the selected cell's building type, while
**Other building** chooses another building or route. **Shuffle neighbors**
changes the preview conditions. Conditioned geometry is normalized against the
unconditioned model so changing neighbor conditions does not rescale the shared
building geometry. Mesh View shows a hex outline for the planet cell boundary
and frames that guide at the same apparent scale as the interaction preview.

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
