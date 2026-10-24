"""Rythmes MIDI pour make it drum : pad i = note 36 + i (BOOM 36 … CHAOS 55)."""
import random, struct, sys, os

N = {n: 36 + i for i, n in enumerate(
    "BOOM KICK THUD SNAP CRACK BUZZ RING TICK GLASS WASH BEND SWELL STUTTER CRUSH PHASE GROWL PERC BELL DARK CHAOS".split())}
PPQ = 96           # ticks par noire
STEP = PPQ // 4    # un seizième

def vlq(n):
    out = [n & 0x7F]; n >>= 7
    while n: out.append(0x80 | (n & 0x7F)); n >>= 7
    return bytes(reversed(out))

def write_mid(path, name, events):
    """events : liste de (tick, note, vel, duree_ticks)."""
    msgs = []
    for t, note, vel, dur in events:
        msgs.append((t, 0, bytes([0x99, note, max(1, min(127, vel))])))
        msgs.append((t + dur, -1, bytes([0x89, note, 0])))      # off trié avant les on au même tick
    msgs.sort(key=lambda m: (m[0], m[1]))
    data = b"\x00\xFF\x03" + vlq(len(name.encode())) + name.encode()
    last = 0
    for t, _, m in msgs:
        data += vlq(t - last) + m; last = t
    end = 4 * 4 * PPQ   # 4 mesures
    data += vlq(max(0, end - last)) + b"\xFF\x2F\x00"
    with open(path, "wb") as f:
        f.write(b"MThd" + struct.pack(">IHHH", 6, 0, 1, PPQ))
        f.write(b"MTrk" + struct.pack(">I", len(data)) + data)

def grid(pattern, note, vel=100, accent=None, dur=1, bars=4, swing=0.0):
    """pattern : chaîne de 16 caractères par mesure ('x' frappe, 'o' frappe douce, '.' rien)."""
    ev = []
    pats = pattern.split("|")
    for bar in range(bars):
        p = pats[bar % len(pats)]
        for i, c in enumerate(p):
            if c == ".": continue
            v = vel if c == "x" else int(vel * 0.55)
            if accent and i in accent: v = min(127, v + 20)
            t = (bar * 16 + i) * STEP
            if swing and i % 2 == 1: t += int(STEP * swing)
            ev.append((t, note, v, dur * STEP))
    return ev

rhythms = {}

# 01 — quatre au plancher : KICK, SNAP sur 2 et 4, TICK en croches, GLASS en contretemps
rhythms["01 quatre au plancher"] = (
    grid("x...x...x...x...", N["KICK"], 110)
  + grid("....x.......x...", N["SNAP"], 105)
  + grid("x.x.x.x.x.x.x.x.", N["TICK"], 80, accent={0, 8})
  + grid("..x...x...x...x.", N["GLASS"], 60)
  + grid("...............x|................|................|.........x..x.x.", N["PERC"], 85)
)

# 02 — boom bap : BOOM syncopé, CRACK caisse claire, TICK swingué, ghost notes PERC
rhythms["02 boom bap"] = (
    grid("x.....x..x......|x.....x...x.....", N["BOOM"], 115)
  + grid("....x.......x...|....x.......x..o", N["CRACK"], 110)
  + grid("x.x.x.x.x.x.x.x.", N["TICK"], 75, accent={0, 4, 8, 12}, swing=0.33)
  + grid("...o.......o..o.", N["PERC"], 70, swing=0.33)
  + grid("..............x.|................", N["THUD"], 90)
)

# 03 — breakbeat : THUD grave, SNAP sur 2 et 4 + fantômes, TICK en doubles croches
rhythms["03 breakbeat"] = (
    grid("x..x......x.x...|x..x.....x..x...", N["THUD"], 112)
  + grid("....x..o..o.x..o|....x..o.o..x...", N["SNAP"], 108)
  + grid("xoxoxoxoxoxoxoxo", N["TICK"], 70, accent={0, 4, 8, 12})
  + grid("..........x.....|............x...", N["RING"], 85)
  + grid("...............x|...............x|...............x|.............xxx", N["CRUSH"], 90)
)

# 04 — half-time : BOOM, SNAP sur le 3, hi-hats TICK avec roulements STUTTER, BELL
rhythms["04 half-time"] = (
    grid("x......x..x.....|x.........x..x..", N["BOOM"], 118)
  + grid("........x.......", N["SNAP"], 112)
  + grid("x.x.x.x.x.x.x.x.|x.x.x.x.x.x.xxxx|x.x.x.x.x.x.x.x.|x.x.xxx.x.x.xxxx", N["TICK"], 72)
  + grid("................|................|......xxxx......|..............xx", N["STUTTER"], 65)
  + grid("...x.........x..|................", N["BELL"], 80)
  + grid("................|................|................|x...............", N["DARK"], 95, dur=8)
)

# 05 — polyrythme 3 contre 4 : RING toutes les 3 doubles, WASH toutes les 4, DARK au début
poly = []
for s in range(0, 64, 3):
    poly.append((s * STEP, N["RING"], 95 if s % 16 == 0 else 78, STEP))
for s in range(0, 64, 4):
    poly.append((s * STEP, N["WASH"], 85, 3 * STEP))
for s in range(0, 64, 16):
    poly.append((s * STEP, N["DARK"], 110, 8 * STEP))
for s in range(8, 64, 16):
    poly.append((s * STEP, N["BEND"], 90, 4 * STEP))
rhythms["05 polyrythme 3 contre 4"] = poly

# 06 — chaos contrôlé : les 20 pads, tirage reproductible, pulsation BOOM conservée
rng = random.Random(42)
chaos = grid("x.......x.......", N["BOOM"], 115) + grid("....x.......x...", N["SNAP"], 100)
for s in range(64):
    if rng.random() < 0.35:
        note = 36 + rng.randrange(20)
        chaos.append((s * STEP, note, rng.randrange(55, 110), STEP * rng.choice([1, 1, 2, 4])))
rhythms["06 chaos"] = chaos

# 00 — kit perso actuel : pads 6, 7 et 11 (notes 41, 42, 46)
rhythms["00 kit perso (pads 6, 7, 11)"] = (
    grid("x......x..x.....", 41, 110)
  + grid("....x.......x...", 42, 100)
  + grid("x.x.x.x.x.x.x.x.", 46, 70, accent={0, 8})
)

out = os.path.expanduser(sys.argv[1])
os.makedirs(out, exist_ok=True)
for name, ev in rhythms.items():
    write_mid(os.path.join(out, name + ".mid"), name, ev)
    print(f"{name}: {len(ev)} notes")
