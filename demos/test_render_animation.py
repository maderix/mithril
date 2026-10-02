"""Checks for the animation source adapter and image packaging.

Generated execution and full-frame CPU/GPU equality are checked by the renderer.
Run: python3 -m unittest discover -s demos -p test_render_animation.py
"""
import ast
import tempfile
from pathlib import Path
import unittest

from render_animation import ROOT, animated_source, encode, image_bytes


class AnimationTests(unittest.TestCase):
    def test_both_tracers_have_valid_calls_and_keep_quality_settings(self):
        for name, width, spp in [("whitted", 512, None), ("path", 256, 64)]:
            with self.subTest(name=name):
                source = (ROOT / f"demos/cornell_{name}.py").read_text()
                tree = ast.parse(animated_source(source, 24))
                funcs = {f.name: f for f in tree.body if isinstance(f, ast.FunctionDef)}
                self.assertEqual(ast.literal_eval(funcs["size"].body[0].value), width)
                if spp:
                    self.assertEqual(ast.literal_eval(funcs["spp"].body[0].value), spp)
                for n in ast.walk(tree):
                    if isinstance(n, ast.Call) and isinstance(n.func, ast.Name) and n.func.id in funcs:
                        self.assertEqual(len(n.args), len(funcs[n.func.id].args.args), n.func.id)
                self.assertEqual(len(funcs["main"].args.args), 0)
                self.assertEqual(len(funcs["dot"].args.args), 2)
                self.assertEqual(funcs["scene"].args.args[-1].arg, "frame")
                self.assertEqual(funcs["pixel"].args.args[-1].arg, "frame")
                original = ast.parse(source)
                old = {f.name: f for f in original.body if isinstance(f, ast.FunctionDef)}
                for helper in ["sphere", "block", "channel", "unit"]:
                    self.assertEqual(ast.dump(funcs[helper]), ast.dump(old[helper]))

    def test_small_verification_overrides_do_not_change_defaults(self):
        source = (ROOT / "demos/cornell_path.py").read_text()
        tree = ast.parse(animated_source(source, 4, size=8, spp=2))
        funcs = {f.name: f for f in tree.body if isinstance(f, ast.FunctionDef)}
        self.assertEqual(ast.literal_eval(funcs["size"].body[0].value), 8)
        self.assertEqual(ast.literal_eval(funcs["spp"].body[0].value), 2)
        self.assertIn("return 64", source)

    def test_motion_is_bounded_and_periodic(self):
        tree = ast.parse(animated_source((ROOT / "demos/cornell_path.py").read_text(), 24))
        motion = next(f for f in tree.body if isinstance(f, ast.FunctionDef) and f.name == "motion")
        env = {"f32": float}
        exec(compile(ast.Module(body=[motion], type_ignores=[]), "motion", "exec"), env)
        values = [env["motion"](f) for f in range(25)]
        self.assertEqual(values[0], values[24])
        self.assertEqual(values[12], 1.0)
        self.assertTrue(all(-1.0 <= v <= 1.0 for v in values))
        self.assertGreater(len(set(values)), 8)

    def test_objects_move_in_three_dimensions_without_intersection(self):
        tree = ast.parse(animated_source((ROOT / "demos/cornell_whitted.py").read_text(), 24))
        funcs = {f.name: f for f in tree.body if isinstance(f, ast.FunctionDef)}
        env = {"f32": float}
        exec(compile(ast.Module(body=[funcs["motion"]], type_ignores=[]), "motion", "exec"), env)
        spheres = [n for n in ast.walk(funcs["scene"]) if isinstance(n, ast.Call)
                   and isinstance(n.func, ast.Name) and n.func.id == "sphere"]
        positions = {}
        for sphere in spheres:
            material = ast.literal_eval(sphere.args[4])
            radius = ast.literal_eval(sphere.args[3])
            code = compile(ast.Expression(sphere.args[2]), "center", "eval")
            points = [eval(code, dict(env, frame=f)) for f in range(24)]
            positions[material] = points
            for axis in range(3):
                self.assertGreater(max(p[axis] for p in points) - min(p[axis] for p in points), 0.05)
            self.assertTrue(all(-1.0 - 1e-7 <= x - radius and x + radius <= 1.0 + 1e-7
                                for p in points for x in p))
            if material == 5:
                self.assertTrue(all(p[0] + radius < 0.1 for p in points))
            else:
                self.assertTrue(all(p[2] + radius < 0.25 for p in points))
        for mirror, glass in zip(positions[5], positions[6]):
            self.assertGreater(sum((a - b) ** 2 for a, b in zip(mirror, glass)), (0.4 + 0.35) ** 2)
        calls = [n for n in ast.walk(funcs["sample"]) if isinstance(n, ast.Call)
                 and isinstance(n.func, ast.Name) and n.func.id == "trace"]
        self.assertEqual(ast.literal_eval(calls[0].args[0]), (0.0, 0.0, -3.2))

    def test_packed_rgb_is_decoded_without_channel_loss(self):
        w, rgb = image_bytes("(1, 4, [16711680, 65280, 255, 16777215])", 4)
        self.assertEqual(w, 1)
        self.assertEqual(rgb, bytes([255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]))

    def test_bad_images_are_rejected(self):
        for text in ["", "(0, 0, [])", "(1, 3, [1, 2, 3])", "(1, 4, [1])",
                     "(1, 4, [-1, 0, 0, 0])", "(1, 4, [16777216, 0, 0, 0])"]:
            with self.subTest(text=text), self.assertRaises(ValueError):
                image_bytes(text, 4)

    def test_gif_retains_frame_order_timing_and_loop(self):
        from PIL import Image
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "animation.gif"
            rgb = bytes([255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255])
            encode(rgb, 1, 4, out)
            with Image.open(out) as im:
                self.assertEqual(im.n_frames, 4)
                self.assertEqual(im.info["loop"], 0)
                for f, color in enumerate([(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 255)]):
                    im.seek(f)
                    self.assertEqual(im.info["duration"], 80)
                    self.assertEqual(im.convert("RGB").getpixel((0, 0)), color)
            self.assertTrue(out.with_suffix(".png").exists())


if __name__ == "__main__":
    unittest.main()
