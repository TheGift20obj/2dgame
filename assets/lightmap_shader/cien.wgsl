struct Light {
    pos: vec2<f32>,
    strength: f32,
};

struct Wall {
    pos: vec2<f32>,
    half_size: vec2<f32>,
    height: f32,
};

@group(0) @binding(0)
var<uniform> light: Light;

@group(0) @binding(1)
var<uniform> wall: Wall;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let frag = in.world_pos.xy;

    let light_dir = normalize(frag - light.pos);
    let light_dist = distance(frag, light.pos);

    // długość cienia rośnie z dystansem
    let shadow_len = wall.height * light_dist * 0.01;

    // punkt rzutu cienia
    let shadow_origin = wall.pos + light_dir * shadow_len;

    // sprawdzamy czy pixel jest "za ścianą"
    let inside =
        abs(frag.x - shadow_origin.x) < wall.half_size.x &&
        abs(frag.y - shadow_origin.y) < wall.half_size.y;

    if (inside) {
        return vec4(0.0, 0.0, 0.0, 1.0); // cień
    }

    return vec4(1.0); // światło
}
