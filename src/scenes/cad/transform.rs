use super::*;

fn vec3_is_finite(value: Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}

fn parse_vec3_input(text: &str, label: &str) -> Result<Vec3, String> {
    let parts = if text.contains(',') {
        text.split(',').collect::<Vec<_>>()
    } else {
        text.split_whitespace().collect::<Vec<_>>()
    };
    if parts.len() != 3 {
        return Err(format!("Enter {label} as three X, Y, Z values"));
    }
    let values = parts
        .into_iter()
        .map(|value| {
            let value = value.trim();
            if value.is_empty() {
                return Err(format!("Enter {label} as three X, Y, Z values"));
            }
            value
                .parse::<f32>()
                .map_err(|_| format!("Invalid {label} value '{value}'"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let value = vec3(values[0], values[1], values[2]);
    if !vec3_is_finite(value) {
        return Err(format!("{label} values must be finite"));
    }
    Ok(value)
}

pub(super) fn parse_transform_values(mode: TransformMode, text: &str) -> Result<Vec3, String> {
    let values = parse_vec3_input(text, "transform")?;
    match mode {
        TransformMode::Scale
            if [values.x, values.y, values.z]
                .iter()
                .any(|value| *value <= 0.0 || *value > MAX_SCALE_FACTOR) =>
        {
            Err(format!(
                "Scale factors must be greater than 0 and at most {MAX_SCALE_FACTOR}"
            ))
        }
        TransformMode::Move | TransformMode::Rotate
            if [values.x, values.y, values.z]
                .iter()
                .any(|value| value.abs() > MAX_TRANSFORM_VALUE) =>
        {
            Err(format!(
                "Transform values must be between -{MAX_TRANSFORM_VALUE} and {MAX_TRANSFORM_VALUE}"
            ))
        }
        _ => Ok(values),
    }
}

pub(super) fn constrained_transform_values(
    mode: TransformMode,
    axis: TransformAxis,
    values: Vec3,
) -> Vec3 {
    let unchanged = if mode == TransformMode::Scale {
        1.0
    } else {
        0.0
    };
    match axis {
        TransformAxis::All => values,
        TransformAxis::X => vec3(values.x, unchanged, unchanged),
        TransformAxis::Y => vec3(unchanged, values.y, unchanged),
        TransformAxis::Z => vec3(unchanged, unchanged, values.z),
    }
}

pub(super) fn transform_position(position: Vec3, command: TransformCommand) -> Vec3 {
    match command.mode {
        TransformMode::Move => position + command.values,
        TransformMode::Rotate => {
            let mut relative = position - command.pivot;
            let radians = [
                command.values.x.to_radians(),
                command.values.y.to_radians(),
                command.values.z.to_radians(),
            ];
            let (sin_x, cos_x) = radians[0].sin_cos();
            relative = vec3(
                relative.x,
                relative.y * cos_x - relative.z * sin_x,
                relative.y * sin_x + relative.z * cos_x,
            );
            let (sin_y, cos_y) = radians[1].sin_cos();
            relative = vec3(
                relative.x * cos_y + relative.z * sin_y,
                relative.y,
                -relative.x * sin_y + relative.z * cos_y,
            );
            let (sin_z, cos_z) = radians[2].sin_cos();
            relative = vec3(
                relative.x * cos_z - relative.y * sin_z,
                relative.x * sin_z + relative.y * cos_z,
                relative.z,
            );
            command.pivot + relative
        }
        TransformMode::Scale => {
            let relative = position - command.pivot;
            command.pivot
                + vec3(
                    relative.x * command.values.x,
                    relative.y * command.values.y,
                    relative.z * command.values.z,
                )
        }
    }
}

pub(super) fn apply_transform_to_indices(
    mesh: &mut EditableMesh,
    indices: &[usize],
    command: TransformCommand,
) -> Result<(), String> {
    if indices.is_empty() {
        return Err("Select vertices, edges, or faces before transforming".to_string());
    }
    if !vec3_is_finite(command.pivot) || !vec3_is_finite(command.values) {
        return Err("Transform values must be finite".to_string());
    }
    if command.mode == TransformMode::Scale
        && [command.values.x, command.values.y, command.values.z]
            .iter()
            .any(|value| *value <= 0.0 || *value > MAX_SCALE_FACTOR)
    {
        return Err("Scale factors must be greater than 0 and at most 1000".to_string());
    }

    let transformed = indices
        .iter()
        .map(|&index| {
            let vertex = mesh
                .vertices
                .get(index)
                .ok_or_else(|| format!("Selected vertex {index} is out of range"))?;
            let position = transform_position(vertex.position, command);
            if !vec3_is_finite(position) {
                return Err("Transform would create non-finite vertex coordinates".to_string());
            }
            Ok((index, position))
        })
        .collect::<Result<Vec<_>, String>>()?;

    for (index, position) in transformed {
        mesh.vertices[index].position = position;
    }
    Ok(())
}

pub(super) fn transform_has_effect(command: TransformCommand) -> bool {
    match command.mode {
        TransformMode::Move => command.values.length_squared() > 1e-8,
        TransformMode::Rotate => command.values.length_squared() > 1e-8,
        TransformMode::Scale => (command.values - Vec3::ONE).length_squared() > 1e-8,
    }
}
