"""Check icon assets and concrete component coverage without host SDK dependencies."""
from pathlib import Path
import json, re, struct, xml.etree.ElementTree as ET
root=Path(__file__).resolve().parents[1]
assets=root/'assets/icons'
manifest=json.loads((assets/'manifest.json').read_text())
for name in manifest:
    data=(assets/(name+'.png')).read_bytes()
    assert data[:8]==b'\x89PNG\r\n\x1a\n', name
    assert struct.unpack('>II',data[16:24])==(24,24), name
    ET.parse(assets/(name+'.svg'))
if (root/'dotnet/Xvarna.Grasshopper').exists():
    for host in ['Xvarna.Grasshopper','Xvarna.Grasshopper2']:
        count=0
        for file in (root/'dotnet'/host/'Components').rglob('*.cs'):
            text=file.read_text(encoding='utf-8-sig')
            for component in re.findall(r'public (?:sealed )?(?:partial )?class (\w+Component)\s*:',text):
                assert component.removesuffix('Component') in manifest,component
                count+=1
        assert count==49,(host,count)
else:
    text=(root/'varjam-gh/VarjamSuite.cs').read_text(encoding='utf-8-sig')
    for name in re.findall(r'Icon => VarjamIcons\.(\w+)',text): assert name in manifest,name
print(f'PASS: {len(manifest)} PNG/SVG pairs, dimensions and component coverage')
