# A face animation is the values of the rig's controls, evaluated into facial joint poses and morph weights

Decided for issue #154, which also does the facial-joint part of #533. Evidence: `p4kx chf glb` over
`masculinedefault.chf` and `femininedefault.chf` of LIVE, with the animations
`facerom_t0_face`, `ai_civ_neutral_idle_08_face` and `nw_neutral_stand_getupback`
(`crates/p4kx-assembly/tests/real_archive/real_head_animation.rs`): all 141 facial animations of
`Animations/Characters/Human/heads/male` evaluate without an issue, and the GLBs pass the checker
of the testkit. The layouts of the rig logic and of the delta records are format knowledge from the
MIT-licensed StarBreaker project, checked against the Archive.

## What a face animation is

A face `.caf` has one track per control of the head rig. The track's controller ID is the CRC-32
(IEEE) of the control's name as the `RigLogic` chunk of the head skin spells it, e.g.
`CTL_expressions blinkL`; 172 of the 183 tracks of an idle clip and 87 of 88 of a range-of-motion
clip name a control. The control's value, 0 to 1, is the first component of the track's translation
keys; the rotation keys are a constant. The frames are 30 per second. The reader of ADR-0046
decodes these tracks without change.

Amendment (#565): the chunk also stores the table of these hashes, one per control (slot 1 of the
rig, in front of the name pool). `RigLogic::read` finds the controls by the stored hashes and
refuses a chunk whose table is not the CRC-32 of the names. On LIVE all 130 skins with a
`RigLogic` chunk agree (`real_rig_logic.rs`).

## Why the facial joints are needed (#533)

The 495 blendshapes of ADR-0091 move vertices by millimetres (at most 2.5 mm for `blinkL`). The
joints do the large part: for the peak frame of `facerom_t0_face` the delta records move
`FACIAL_LOD1_R_NasolabialFurrow` by 26 mm and turn it by 0.37 rad, and the 15 strongest joints move
by 1 to 3 cm. A face animation of morph weights alone would hardly move, so the facial joints are
part of the skeleton now.

## The skeleton

`p4kx_model::extend_skeleton` appends to the body skeleton the bones of the head skin it lacks (183
of 198, matched by name ignoring case), in the skin's order. Each keeps the skin's bind pose relative
to its parent, moved by the Character's own joint offset, and hangs below its nearest ancestor of
the skin that the result has (`FACIAL_C_FacialRoot` below `Head`). The world poses follow from the
parents' by forward kinematics, so the inverse bind matrices are consistent with the skeleton the
GLB writes whatever the skin's own world poses say; the head mesh is not moved, the joints are
re-bound to the shaped face. The skeleton has 403 bones with a head at LOD 0 (220 and 183). The
weights of the head bind to the facial joints by name (ADR-0079), so `bones_not_in_skeleton` no
longer appears for the head.

The rig's 179 joints are the bones whose CRC-32 is listed in section F of the `RigLogic` chunk, in
the order of the joints of the DNA rig; all 179 are found among the bones of the skin. The offset of
a joint is `DnaRig::neutral_joint_offsets` (slot 15, folded by the joint weights of slot 21), added
to its parent-local translation. Only LOD 0 has the data: the rigs of the lower levels have only
the neutral morphs and region masks, so the heads of other levels are extended without offsets and
cannot be animated.

## The evaluation

`FaceRig` (p4kx-character) holds, per joint axis, the blendshapes that move it and their summed
coefficients. A control animation is turned into the 495 activations: the controls, with the
constant control 0 at 1, then each corrective as the product of the five controls of its row in
section C. The translation of a joint is the sum over the records of the main delta set, the Euler
angles (radians) the sum over the secondary set, each coefficient times the step of its head
(`scale / (2^22 - 1)`) times the activation, mixed over the heads of the DNA by the joint weights
of the regions. What the heads leave of 1 in a region belongs to the base head, as for the
expression morphs (ADR-0091); a region without blends has the records of the base head.

The Euler angles `x, y, z` become the quaternion `qx * qy * qz`, the Hamilton product of the
half-angle quaternions with `qx` leftmost, and the pose of the joint is its bind translation plus
the translation, and its bind rotation times that quaternion on the right (`bind * delta`: the
delta turns in the joint's own axes). Order, signs and side are those StarBreaker documents for the
engine; they are tested for what the code does (two axes, a rotated bind pose).

Amendment (#565). The rotations were checked on LIVE against motions whose direction is known (`real_head_rotation.rs`, clip
`facerom_t0_face`, forward kinematics over the skeleton, the probe joint a descendant of the
rotated one). With the convention above, `eyeLookUp` moves the pupil up by 6 mm and `eyeLookDown`
down by 3.5 mm, `blinkL` and `blinkR` move the upper eyelid down by 7 mm, and `CTL_C_jaw ty` moves
the chin down by 26 mm and back by 20 mm, as the names say. Applying the inverse rotation
(`bind * inverse(delta)` or `inverse(delta) * bind`) moves every one of these the opposite way, so
the **sign of the angles** is settled. The **Euler order** (XYZ against ZYX would move a joint by
up to 14 degrees on this clip, most at the eyeballs and the lower lip) and the **side** of the
product (`delta * bind` against `bind * delta` by up to 44 degrees, most at the nostrils and the
lip corners) are not: the motions with a known direction do not depend on them at all (eyes, jaw)
or by under 1 mm (eyelids). They rest on StarBreaker's reading of the engine
(`euler_xyz_to_quat` as `qx * qy * qz`, the sin/cos chain at `0x1466e4dbf`, and
`bind.rotation * euler_xyz_to_quat(delta)`), and stay open (#627) until an in-game or engine
reference of one expression with a rotated bind pose, such as a nostril or a lip corner, can be
compared.

The Assembly verification feature evaluates all four combinations of XYZ or ZYX and bind-first or
delta-first composition at explicit clip frames on one rig. The resulting GLB actions share the
original frame times, translations and morph weights. The private verification records bind each
row of original Euler radians to its CAF frame and mapped RigLogic joint. A candidate remains
unselected until comparison with an independent reference resolves the difference.

On PTU 13284307 the audio of a captured MFD mission dialogue identifies one authored take from
66 examined takes. Its associated face CAF retains 1,213 frame times at 30 fps in each of four
exported actions. The archived DNA rig binds all 179 joints. In the capture, two held-out MFD
markers align within one rectified pixel across eight sampled frames. This fixes the display
plane registration. The CAF-to-video clock, the runtime rig initialization and the projection of
the face still require independent evidence before the candidate comparison can select an order.

The 495 activations are also the weights of the morph targets: the targets of the head are the
blendshapes that move a vertex, in the order of their numbers, and the build keeps the number of
each, so target `k` takes the activation of blendshape `numbers[k]`.

## Sampling

An animation is sampled at the frames where any of its control tracks has a key, because the
inputs change linearly between keys and the viewers interpolate translations and weights linearly
and rotations spherically. Control values are held
to 0 to 1, since a control is a fraction and a file with a key of 1e30 must not blow up the pose; a
pose that is not finite refuses the clip. A sample is dropped when the line between its kept
neighbours misses it by no more than 0.5 mm of a joint translation, 0.001 per quaternion
component (about 0.1 to 0.2 degrees) and 0.01 of a weight on every output; a line spans at most 128 samples and the
samples are reduced 1,024 at a time, so the work is linear. The face clips key nearly every frame
(`ai_civ_neutral_idle_08_face` has 4,354 samples of its 4,530 frames), so the reduction needs
tolerances that are coarse against the keys' noise to save anything.

Amendment (#565), the tolerances were 0.1 mm, 0.0001 and 0.001 and are now those above. Measured
on LIVE: `ai_civ_neutral_idle_08_face` (151 s) keeps 2,275 samples instead of 4,352 and its GLB is
20.7 MB instead of 34.3 MB (40 % less); `facerom_t0_face` (28 s) keeps 681 instead of 827 and 9.5
MB instead of 10.5 MB. The error is the stated tolerance, at most, at every dropped sample and
every output, and a viewer interpolating between kept samples is within it there (for rotations
the straight line and the slerp differ negligibly over such small angles); 0.5 mm
is a twentieth of the 1 to 3 cm the strongest joints move. A coarser tolerance saves more (2 mm,
0.002 and 0.01 keep 1,794 samples of the idle clip) but starts to show on the eyelids, which move
by 7 mm. A separate, sparser time axis for the weights alone was measured and not made: reduced on
their own, the weights of the idle clip keep 3,765 of 4,529 samples at the old tolerance and 1,574
at ten times that, as many as the joints do, so a second time axis would save little.

Joints that never leave their bind pose and morph targets that never leave 0 get no channel; the
weights are one `weights` channel of the head's mesh with a run of all targets per sample.

## The budget

The animations of one Character together may write 33,554,432 values (floats of key times, poses and
weights, 128 MiB; `MAX_ANIMATION_VALUES`), and one facial clip at most 32,768 samples. The 141
facial clips of `heads/male` alone come to about 150 Mi values, so `--anim all` writes the clips
that fit in the order of the files and lists the others as `animation_not_used` with a reason
starting with `limit`; the cost of a facial clip is estimated before the rig is evaluated, so a
refused clip costs no memory. Run `--anim` with the names wanted for more.

## What is not in it

- Slot 17 of the rig, the per-vertex attenuation of the blendshapes: not applied, the targets are
  the cells of slot 19 as in ADR-0091 (ADR-0293). ADR-0281 compares the `BShapesGpu` chunk with
  the cells for the base head and uses it for heads without a DNA rig.
- The joints of the lower levels. Their morph targets are those of level 0 carried over
  (ADR-0293).
- The wrinkle maps that sections G and H of the rig logic drive.
- Verification of the Euler order and the multiplication side against the engine (#627; the sign
  is settled, see the amendment of #565).
- One glTF animation per clip: an animation with tracks for bones and for controls is written as
  two animations of the same name.

## Body animations and where they are found

Body animations are the `.caf` files and `.dba` databases below the folders that the skeleton's
parameters list (`bhm_skeleton_v7.chrparams`, `bhf_skeleton_v2.chrparams`): `Animations/Characters/Human/<body>`,
`Animations/pu/<gender>` and `Animations/Characters/Human/heads/<gender>`, nested folders included
(`BodyVariant::animation_folders`). The parameters themselves are still not read: the folders are
constants of the variant, like its skeleton and skin. `Armour::animate` fits each animation: tracks
whose controller ID is that of a bone of the skeleton move it by their keys (ADR-0046), tracks
whose controller ID is that of a control of the head rig are evaluated as above, and the controller
IDs that fit neither are reported in the summary of the GLB, in `skipped_tracks`, together with
the IDs of the face rig's joints that move but are not in the skeleton (#565): both are the CRC-32
of a name and the field does not tell them apart. An animation that fits nothing is not
written and is listed as `animation_not_used`. `p4kx chf glb` and `p4kx armour glb` take `--anim`
and `--anim-from`. Additive animations that nothing names are no longer listed one by one when a
name picks the animations (a character's folders hold hundreds).

Amendment (#584): animations left out for the same reason are one `animation_not_used` issue with
their count and the names of the first three, not one issue each.

## Considered options

- Morph weights only: millimetres of motion, the face would not move.
- Bake the joints into more morph targets: the targets would hold the joint motion and the
  skin could not follow the head.
- Sample every frame: larger files for nothing, the inputs have keys.
- Sample at a fixed lower rate: loses the keys the files hold.
