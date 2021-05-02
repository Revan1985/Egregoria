#version 450
#include "render_params.glsl"

layout(location=0) in vec4 in_tint;
layout(location=1) in vec3 in_normal;
layout(location=2) in vec3 in_wpos;
layout(location=3) in vec2 in_uv;

layout(location=0) out vec4 out_color;

layout(set = 1, binding = 0) uniform Uni {RenderParams params;};

layout(set = 2, binding = 0) uniform texture2D t_albedo;
layout(set = 2, binding = 1) uniform sampler s_albedo;

layout(set = 3, binding = 0) uniform texture2D t_ssao;
layout(set = 3, binding = 1) uniform sampler s_ssao;

layout(set = 3, binding = 2) uniform texture2D t_bnoise;
layout(set = 3, binding = 3) uniform sampler s_bnoise;

layout(set = 3, binding = 4) uniform texture2D t_sun_smap;
layout(set = 3, binding = 5) uniform samplerShadow s_sun_smap;

float dither() {
    float color = texture(sampler2D(t_bnoise, s_bnoise), gl_FragCoord.xy / 512.0).r;
    return (color - 0.5) / 255.0;
}

float sampleShadow(vec3 wpos) {
    vec4 light_local = params.sunproj * vec4(wpos, 1);
    if (light_local.w <= 0.0) {
        return 1.0;
    }
    vec3 corrected = light_local.xyz / light_local.w * vec3(0.5, -0.5, 1.0) + vec3(0.5, 0.5, 0.0);

    float v = texture(sampler2DShadow(t_sun_smap, s_sun_smap), corrected);

    return mix(v, 1, clamp(dot(light_local.xy, light_local.xy), 0.0, 1.0));
}

void main() {
    vec4 albedo = texture(sampler2D(t_albedo, s_albedo), in_uv);
    float ssao = 1;
    if (params.ssao_enabled != 0) {
       ssao = texture(sampler2D(t_ssao, s_ssao), gl_FragCoord.xy / params.viewport).r;
/*
        if (gl_FragCoord.x > params.viewport.x * 0.5) {
            out_color = vec4(vec3(ssao), 1);
            return;
        }*/
    }

    float shadow_v = 1;

    if (params.shadow_mapping_enabled != 0) {
        shadow_v = sampleShadow(in_wpos);
    }

    /*
    out_color = vec4(in_wpos * 0.001, 1);
    return;
    */
    /*
    vec2 p = gl_FragCoord.xy;
    if (p.x < 500 && p.y < 500) {
        out_color = vec4(vec3(texture(sampler2DShadow(t_sun_smap, s_sun_smap), vec3(p / 500, 1))), 1);
        return;
    }    */

    vec3 normal = normalize(in_normal);
    vec3 cam = params.cam_pos.xyz;

    vec3 L = params.sun.xyz;
    vec3 R = normalize(2 * normal * dot(normal,L) - L);
    vec3 V = normalize(cam - in_wpos);

    float specular = clamp(dot(R, V), 0.0, 1.0);
    specular = pow(specular, 5);

    float diffuse = clamp(dot(normal, params.sun.xyz), 0.0, 1.0);

    vec4 c = in_tint * albedo;
    c.rgb *= 0.2 + 0.8 * diffuse * shadow_v + 0.5 * specular;
    c.rgb *= ssao;
    c.rgb += dither();
    out_color = c;
}