import sys

beats_per_minute = 100
beat = 60.0 / beats_per_minute
bar = beat * 4

note_offsets = {"C": 0, "C#": 1, "D": 2, "D#": 3, "E": 4, "F": 5, "F#": 6, "G": 7, "G#": 8, "A": 9, "A#": 10, "B": 11}


def frequency(name):
    midi = 12 * (int(name[-1]) + 1) + note_offsets[name[:-1]]
    return 440.0 * 2 ** ((midi - 69) / 12)


chords = [
    {"notes": ["D#4", "F#4", "G#4", "B4", "D#5"], "bass": "G#1"},
    {"notes": ["E4", "F#4", "G#4", "B4", "D#5"], "bass": "E1"},
    {"notes": ["D#4", "F#4", "G#4", "B4", "C#5"], "bass": "B1"},
    {"notes": ["D#4", "F#4", "A#4", "C#5", "F#5"], "bass": "F#1"},
]

slow_progression = [0, 0, 1, 1, 2, 2, 3, 3]
fast_progression = [0, 1, 2, 3]

groove = {"horn", "pump", "kick", "hats"}
verse = groove | {"snare", "bass"}
chorus = verse | {"ticks", "lead"}

arrangement = [
    {"bars": 8, "progression": slow_progression, "parts": groove},
    {"bars": 16, "progression": slow_progression, "parts": verse},
    {"bars": 16, "progression": fast_progression, "parts": chorus, "arpeggio": (0.0, 0.7)},
    {"bars": 6, "progression": slow_progression, "parts": {"horn"}, "arpeggio": (0.7, 1.0)},
    {"bars": 2, "progression": [3], "parts": {"horn", "roll"}, "arpeggio": (1.0, 0.4)},
    {"bars": 16, "progression": fast_progression, "parts": chorus, "arpeggio": (0.4, 1.0)},
    {"bars": 8, "progression": slow_progression, "parts": verse, "arpeggio": (0.8, 0.0)},
]

bars = []
for section_index, part_of_song in enumerate(arrangement):
    ramp = part_of_song.get("arpeggio")
    count = part_of_song["bars"]
    for index in range(count):
        parts = set(part_of_song["parts"])
        entry = {
            "chord": chords[part_of_song["progression"][index % len(part_of_song["progression"])]],
            "parts": parts,
            "section": section_index,
            "position": index,
            "last_in_section": index == count - 1,
        }
        if ramp:
            parts.add("arpeggio")
            entry["arpeggio"] = (ramp[0] + (ramp[1] - ramp[0]) * index / count,
                                 ramp[0] + (ramp[1] - ramp[0]) * (index + 1) / count)
        bars.append(entry)

length = bar * len(bars)


def bars_with(part):
    return [(index * bar, entry) for index, entry in enumerate(bars) if part in entry["parts"]]


def envelope(curve, points):
    first_value = points[0][0]
    points = points + [(first_value, length)]
    return curve + " " + " ".join("%g@%.4f" % (value, time) for value, time in points)


def hit_times(part, offsets):
    return [start + offset for start, _ in bars_with(part) for offset in offsets]


def percussive_gate(times, shape, scales=None):
    points = [(0, 0.0)]
    for index, time in enumerate(times):
        scale = scales[index] if scales else 1.0
        next_time = times[index + 1] if index + 1 < len(times) else length
        points.append((0, time))
        for value, delay in shape:
            if time + delay >= next_time - 0.001:
                points.append((0, next_time - 0.001))
                break
            points.append((round(value * scale, 4), time + delay))
    return envelope("linear", points)


every_beat = [beat * index for index in range(4)]
offbeats = [beat * index + beat / 2 for index in range(4)]
backbeats = [beat, beat * 3]
eighths = [beat / 2 * index for index in range(8)]
sixteenths = [beat / 4 * index for index in range(16)]
in_between_sixteenths = [beat / 4 * index for index in range(16) if index % 2]


def pad_pitch(voice):
    return envelope("step", [(frequency(entry["chord"]["notes"][voice]), index * bar) for index, entry in enumerate(bars)])


def pad_gate():
    points = [(1, 0.0)]
    for time in hit_times("pump", every_beat):
        points += [(1, time), (0.15, time + 0.01), (1, time + 0.45)]
    return envelope("linear", points)


def kick_pitch():
    points = [(45, 0.0)]
    for time in hit_times("kick", every_beat):
        points += [(45, time), (170, time + 0.001), (55, time + 0.07), (45, time + 0.15)]
    return envelope("log", points)


def stepped_pitch(part, offsets, note_for_step):
    points = [(note_for_step(bars[0], 0), 0.0)]
    step = 0
    for start, entry in bars_with(part):
        for offset in offsets:
            points.append((note_for_step(entry, step), start + offset))
            step += 1
    return envelope("step", points)


verse_bass_pattern = [0, None, 12, 0, None, 7, 0, None, 12, None, 0, 7, None, 12, 2, None]
chorus_bass_pattern = [0, 0, 12, 0, 7, 0, 12, 0, 0, 0, 12, 0, 7, 12, 2, 0]


def bass_events():
    events = []
    for index, entry in enumerate(bars):
        if "bass" not in entry["parts"]:
            continue
        root = frequency(entry["chord"]["bass"])
        pattern = list(chorus_bass_pattern if "lead" in entry["parts"] else verse_bass_pattern)
        following = bars[index + 1] if index + 1 < len(bars) else None
        if following and "bass" in following["parts"] and following["chord"] is not entry["chord"]:
            pattern[14] = 7
            pattern[15] = ("next", 12)
        for step, offset in enumerate(pattern):
            if offset is None:
                continue
            if isinstance(offset, tuple):
                note = frequency(following["chord"]["bass"]) * 2 ** (offset[1] / 12)
            else:
                note = root * 2 ** (offset / 12)
            events.append((index * bar + sixteenths[step], note))
    return events


def bass_pitch():
    events = bass_events()
    points = [(events[0][1], 0.0)] + [(note, time) for time, note in events]
    return envelope("step", points)


arpeggio_pattern = [0, 1, 2, 3, 4, 3, 2, 1]


def arpeggio_note(entry, step):
    return frequency(entry["chord"]["notes"][arpeggio_pattern[step % len(arpeggio_pattern)]]) * 2


def arpeggio_scales():
    scales = []
    for _, entry in bars_with("arpeggio"):
        start, end = entry["arpeggio"]
        for index in range(len(sixteenths)):
            scales.append(round(start + (end - start) * index / len(sixteenths), 4))
    return scales


melody = [
    [(0.0, "D#6", 1.5), (1.5, "B5", 1.5), (3.0, "G#5", 1.0)],
    [(0.0, "G#5", 0.5), (0.5, "B5", 0.5), (1.0, "C#6", 0.5), (1.5, "D#6", 0.5),
     (2.0, "E6", 0.25), (2.25, "D#6", 0.25), (2.5, "B5", 0.25), (2.75, "G#5", 0.25), (3.0, "B5", 1.0)],
    [(0.0, "D#6", 2.0), (2.0, "C#6", 1.0), (3.0, "B5", 1.0)],
    [(0.0, "A#5", 1.5), (1.5, "F#5", 1.0), (2.5, "C#6", 1.5)],
]


def melody_events():
    events = []
    for start, entry in bars_with("lead"):
        for beat_offset, note, duration in melody[entry["position"] % len(melody)]:
            events.append((start + beat_offset * beat, frequency(note), duration * beat))
    return events


def melody_pitch():
    events = melody_events()
    return envelope("step", [(events[0][1], 0.0)] + [(note, time) for time, note, _ in events])


def melody_gate():
    points = [(0, 0.0)]
    for time, _, duration in melody_events():
        points += [(0, time), (1, time + 0.01), (0.8, time + 0.1), (0.8, time + duration * 0.85), (0, time + duration * 0.95)]
    return envelope("linear", points)


def fill_bars():
    small, big = [], []
    for index, entry in enumerate(bars):
        if "kick" not in entry["parts"]:
            continue
        following = bars[index + 1] if index + 1 < len(bars) else None
        section_ends = entry["last_in_section"] or not following or "kick" not in following["parts"]
        if section_ends or (entry["position"] + 1) % 8 == 0:
            big.append(index * bar)
        elif (entry["position"] + 1) % 4 == 0:
            small.append(index * bar)
    return small, big


def tom_hits(tom):
    small, big = fill_bars()
    small_fill = {"high": [0], "mid": [1], "low": [2, 3]}
    big_fill = {"high": [0, 1], "mid": [2, 3, 4], "low": [5, 6, 7]}
    times = [start + beat * 3 + beat / 4 * step for start in small for step in small_fill[tom]]
    times += [start + beat * 2 + beat / 4 * step for start in big for step in big_fill[tom]]
    return sorted(times)


def tom_pitch(times, high, low):
    points = [(low, 0.0)]
    for time in times:
        points += [(low, time), (high, time + 0.001), (low, time + 0.12)]
    return envelope("log", points)


def crash_times():
    times = []
    for index, entry in enumerate(bars):
        previous = bars[index - 1] if index else None
        if "kick" in entry["parts"] and (not previous or previous["section"] != entry["section"]):
            times.append(index * bar)
    return times


def roll_scales():
    count = len(hit_times("roll", sixteenths))
    return [round(0.15 + 0.85 * index / max(1, count - 1), 4) for index in range(count)]


reset = {
    "noise": "off",
    "vibrato": "0 0",
    "glide": "0",
    "drive": "0",
    "highpass": "0",
    "lowpass": "0",
    "peak_eq": "0 0 1",
    "level": "0",
    "gate_slew": "0.003",
}

sections = []


def section(name, settings):
    sections.append((name, settings))


section("horn_synthwave", {
    "description": "Train horn that evolves into a synthwave track",
    "length": "%.4f" % length,
    "reverb_time": "2.2",
    "reverb_width": "24",
    "reverb_flutter": "0.25",
    "reverb_damping": "4500",
    "reverb_wet": "-7",
    "wave": "saw",
    "pitch": pad_pitch(0),
    "glide": "0.08",
    "vibrato": "0.04 2",
    "gate": pad_gate(),
    "drive": "3",
    "highpass": "120",
    "lowpass": "3500",
    "peak_eq": "800 4 1.0",
    "level": "-9",
})
for voice, level in [(1, -9), (2, -10), (3, -11), (4, -13)]:
    section("horn_synthwave.horn%d" % voice, {"pitch": pad_pitch(voice), "level": str(level)})

section("horn_synthwave.kick", dict(reset, **{
    "wave": "sine",
    "pitch": kick_pitch(),
    "gate": percussive_gate(hit_times("kick", every_beat), [(1, 0.001), (0.6, 0.06), (0.2, 0.22), (0, 0.4)]),
    "gate_slew": "0.0005",
    "drive": "3",
    "highpass": "25",
    "level": "4",
}))

section("horn_synthwave.kick_click", dict(reset, **{
    "wave": "none",
    "noise": "0",
    "gate": percussive_gate(hit_times("kick", every_beat), [(1, 0.0005), (0, 0.006)]),
    "gate_slew": "0.0002",
    "highpass": "1500",
    "lowpass": "6000",
    "level": "-8",
}))

section("horn_synthwave.snare", dict(reset, **{
    "wave": "none",
    "noise": "0",
    "gate": percussive_gate(hit_times("snare", backbeats), [(1, 0.001), (0.4, 0.06), (0, 0.25)]),
    "gate_slew": "0.0005",
    "drive": "2",
    "highpass": "1000",
    "lowpass": "9000",
    "level": "1",
}))

section("horn_synthwave.snare_body", dict(reset, **{
    "wave": "sine",
    "pitch": "190",
    "gate": percussive_gate(hit_times("snare", backbeats), [(1, 0.001), (0, 0.1)]),
    "gate_slew": "0.0005",
    "level": "-3",
}))

section("horn_synthwave.roll", dict(reset, **{
    "wave": "none",
    "noise": "0",
    "gate": percussive_gate(hit_times("roll", sixteenths), [(1, 0.001), (0.3, 0.04), (0, 0.12)], roll_scales()),
    "gate_slew": "0.0005",
    "drive": "2",
    "highpass": "1000",
    "lowpass": "9000",
    "level": "-2",
}))

for tom, high, low, level in [("high", 240, 160, -1), ("mid", 175, 112, 0), ("low", 125, 78, 1)]:
    times = tom_hits(tom)
    section("horn_synthwave.tom_" + tom, dict(reset, **{
        "wave": "sine",
        "pitch": tom_pitch(times, high, low),
        "gate": percussive_gate(times, [(1, 0.001), (0.6, 0.08), (0.2, 0.25), (0, 0.4)]),
        "gate_slew": "0.0005",
        "noise": "-14",
        "drive": "1.5",
        "highpass": "50",
        "level": str(level),
    }))

section("horn_synthwave.crash", dict(reset, **{
    "wave": "none",
    "noise": "0",
    "gate": percussive_gate(crash_times(), [(1, 0.001), (0.5, 0.15), (0.15, 0.9), (0, 2.2)]),
    "gate_slew": "0.0005",
    "highpass": "4500",
    "lowpass": "12000",
    "level": "-10",
}))

section("horn_synthwave.hats", dict(reset, **{
    "wave": "none",
    "noise": "0",
    "gate": percussive_gate(hit_times("hats", offbeats), [(1, 0.0005), (0.3, 0.02), (0, 0.06)]),
    "gate_slew": "0.0003",
    "highpass": "7000",
    "level": "-8",
}))

section("horn_synthwave.ticks", dict(reset, **{
    "wave": "none",
    "noise": "0",
    "gate": percussive_gate(hit_times("ticks", in_between_sixteenths), [(1, 0.0005), (0, 0.025)]),
    "gate_slew": "0.0003",
    "highpass": "9000",
    "level": "-15",
}))

section("horn_synthwave.bass", dict(reset, **{
    "wave": "saw",
    "pitch": bass_pitch(),
    "gate": percussive_gate([time for time, _ in bass_events()], [(1, 0.002), (0.6, 0.08), (0, 0.13)]),
    "drive": "1.5",
    "lowpass": "900",
    "level": "-3",
}))

section("horn_synthwave.bass_edge", dict(reset, **{
    "wave": "pulse",
    "pulse_width": "0.3",
    "pitch": bass_pitch(),
    "pitch_scale": "2",
    "gate": percussive_gate([time for time, _ in bass_events()], [(1, 0.002), (0.4, 0.05), (0, 0.1)]),
    "highpass": "150",
    "lowpass": "1800",
    "level": "-14",
}))

section("horn_synthwave.arpeggio", dict(reset, **{
    "wave": "pulse",
    "pulse_width": "0.25",
    "pitch": stepped_pitch("arpeggio", sixteenths, arpeggio_note),
    "gate": percussive_gate(hit_times("arpeggio", sixteenths), [(1, 0.002), (0.5, 0.06), (0, 0.12)], arpeggio_scales()),
    "highpass": "200",
    "lowpass": "3000",
    "level": "-10",
}))

section("horn_synthwave.lead", dict(reset, **{
    "wave": "saw",
    "pitch": melody_pitch(),
    "glide": "0.03",
    "vibrato": "0.15 5",
    "gate": melody_gate(),
    "gate_slew": "0.005",
    "drive": "1",
    "highpass": "300",
    "lowpass": "2500",
    "level": "-9",
}))

drum_voices = {"kick", "kick_click", "snare", "snare_body", "roll", "hats", "ticks", "tom_high", "tom_mid", "tom_low", "crash"}

out = sys.stdout
for sound, description, keep_drums in [
    ("rally_music", "Endless loop of horn-synthwave for PSX Rally", True),
]:
    for name, settings in sections:
        voice = name.partition(".")[2]
        if voice in drum_voices and not keep_drums:
            continue
        out.write("[%s]\n" % name.replace("horn_synthwave", sound, 1))
        for key, value in settings.items():
            if key == "description":
                value = description
            out.write("%s = %s\n" % (key, value))
        if "." not in name:
            out.write("rate = 22050\n")
        out.write("\n")
