use crate::primitive::{GlslCtx, Primitive};

const RENDERER_TEMPLATE: &str = include_str!("renderer.glsl");

const UNIFORMS: &str = "\
precision highp float;\n\
uniform vec2  iResolution;\n\
uniform mat4  iWorldTransform;\n\
uniform float iCameraZ;";

pub fn build_fragment_shader(obj: &dyn Primitive) -> String {
    let mut ctx = GlslCtx::new();
    let result = obj.expression("p", &mut ctx);

    let helpers = ctx.helpers.join("\n\n");

    let stmts: String = ctx
        .statements
        .iter()
        .map(|s| format!("    {s}\n"))
        .collect();

    // map() transforms p by the world matrix first, then evaluates the SDF.
    let map_fn = format!(
        "float map(vec3 p) {{\n\
             p = (iWorldTransform * vec4(p, 1.0)).xyz;\n\
         {stmts}\
         \n    return {result};\n\
         }}"
    );

    // Order: uniforms → helpers → map() → renderer body (calcNormal, main, …)
    format!("{UNIFORMS}\n\n{helpers}\n\n{map_fn}\n\n{RENDERER_TEMPLATE}")
}
