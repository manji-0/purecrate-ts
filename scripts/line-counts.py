#!/usr/bin/env python3
"""Line counts of the examples against their idiomatic references
(design/07 §2.2): non-blank, non-comment lines, after rustfmt at width 120
on both sides, so layout does not decide the ratio.

Types are structs, enums, aliases, consts, and trait impls; logic is
functions and inherent impls. The idiomatic reference is `mod idiomatic`
in the example's test. Needs python3 and rustfmt.

    scripts/line-counts.py
"""
import os, re, subprocess, sys, tempfile

def strip(src):
    out, i, n = [], 0, len(src)
    # remove comments (line and block), keep strings
    res=[]; i=0
    while i<n:
        c=src[i]
        if src.startswith('//',i):
            j=src.find('\n',i); j=n if j<0 else j; i=j; continue
        if src.startswith('/*',i):
            j=src.find('*/',i+2); i=j+2; continue
        if c=='"':
            j=i+1
            while j<n and src[j]!='"':
                j+=2 if src[j]=='\\' else 1
            res.append(src[i:j+1]); i=j+1; continue
        if c=="'" and re.match(r"'(\\.|[^\\'])'", src[i:i+4]) :
            # A literal's text could be a brace (`b'{'`); `items` counts
            # braces, so it stands as a letter.
            m=re.match(r"'(\\u\{[0-9a-fA-F]+\}|\\.|[^\\'])'", src[i:]); res.append("'c'"); i+=len(m.group(0)); continue
        res.append(c); i+=1
    return ''.join(res)

def items(src):
    """Top-level items as (header, text) by brace depth."""
    lines = src.split('\n')
    out=[]; depth=0; cur=[]
    for line in lines:
        if depth==0 and not line.strip():
            continue
        cur.append(line)
        depth += line.count('{') - line.count('}')
        if depth==0 and (line.rstrip().endswith('}') or line.rstrip().endswith(';')) :
            out.append('\n'.join(cur)); cur=[]
    if cur: out.append('\n'.join(cur))
    return out

def kind(text):
    head=' '.join(l.strip() for l in text.split('\n') if l.strip() and not l.strip().startswith('#['))
    if re.match(r'(pub(\([a-z]+\))? )?(const )?fn ', head): return 'logic'
    if re.match(r'impl(<[^>]*>)? [A-Za-z_:<>]+ for ', head): return 'types'
    if head.startswith('impl'): return 'logic'
    if head.startswith('use ') or head.startswith('mod '): return 'skip'
    return 'types'

def count(text):
    return sum(1 for l in text.split('\n') if l.strip())


def module(src, name):
    m = re.search(r'^mod ' + name + r' \{\n', src, re.M)
    i, depth = m.end(), 1
    while depth:
        depth += {'{': 1, '}': -1}.get(src[i], 0)
        i += 1
    body = src[m.end():i - 1]
    return '\n'.join(l[4:] if l.startswith('    ') else l for l in body.split('\n'))

def formatted(src):
    with tempfile.NamedTemporaryFile('w', suffix='.rs', delete=False) as f:
        f.write(src)
    subprocess.run(['rustfmt', '--edition', '2021', '--config', 'max_width=120', f.name], check=True)
    out = open(f.name).read()
    os.unlink(f.name)
    return out

def split(src):
    tot = {'types': 0, 'logic': 0}
    for it in items(strip(src)):
        k = kind(it)
        if k != 'skip':
            tot[k] += count(it)
    return tot

root = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')
print('| Example | Idiomatic logic | Constrained logic | Ratio | Types (idiomatic / constrained) |')
print('| --- | --- | --- | --- | --- |')
for name in sorted(os.listdir(os.path.join(root, 'examples'))):
    test = os.path.join(root, 'crates/cli/tests/it', name + '_equivalence.rs')
    lib = os.path.join(root, 'examples', name, 'src/lib.rs')
    if not (os.path.exists(test) and os.path.exists(lib)) or 'mod idiomatic {' not in open(test).read():
        continue
    i = split(formatted(module(open(test).read(), 'idiomatic')))
    c = split(formatted(open(lib).read()))
    print(f"| {name} | {i['logic']} | {c['logic']} | {c['logic'] / i['logic']:.1f}x | {i['types']} / {c['types']} |")
