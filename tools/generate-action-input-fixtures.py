"""Independent Pillow nearest oracle over original, declaratively bounded shapes."""
import json
from fractions import Fraction
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw

dest=Path(__file__).resolve().parents[1]/'tests/fixtures/synthetic/action-input'
dest.mkdir(parents=True,exist_ok=True)
cases=[]
specs=[('narrow',(128,96),(40,20,8,16),(220,230,150)),
       ('wide',(160,100),(20,20,96,20),(110,215,165)),
       ('half-up',(80,64),(25,15,9,16),(240,210,110)),
       ('half-down',(80,64),(25,15,7,16),(240,210,110)),
       ('odd-margin',(128,64),(40,8,32,48),(220,230,150)),
       ('pattern',(160,96),(30,20,60,40),(230,185,180)),
       ('inner-mark',(128,64),(20,12,48,40),(240,240,240)),
       ('large',(1024,1024),(200,140,70,150),(240,200,180)),
       ('color-boundary',(128,64),(30,10,40,40),(120,180,120)),
       ('nearest-row-boundary',(256,128),(60,20,79,64),(240,220,160)),
       ('nearest-column-boundary',(256,128),(60,20,76,74),(240,220,160))]
for name,size,bounds,color in specs:
    image=Image.new('RGB',size,(20,50,90));d=ImageDraw.Draw(image);x,y,w,h=bounds
    if name.startswith('nearest-'):
        d.rectangle((x,y,x+w-1,y+h-1),fill=color)
        for row in range(2,h-2,2):d.line((x+2,y+row,x+w-3,y+row),fill=(20,50,90))
        for column in range(2,w-2,3):d.line((x+column,y+2,x+column,y+h-3),fill=(20,50,90))
    elif name=='pattern':
        d.rectangle((x,y,x+w-1,y+h-1),fill=color);d.rectangle((x+8,y+9,x+w-9,y+h-10),fill=(20,50,90))
    elif name=='inner-mark':
        d.rectangle((x,y,x+7,y+h-1),fill=color);d.rectangle((x+w-8,y,x+w-1,y+h-1),fill=color);d.rectangle((43,31,44,32),fill=color)
    else:d.rectangle((x,y,x+w-1,y+h-1),fill=color)
    # This unsupported decoration is outside the declarative word bounds.
    d.rectangle((1,1,2,2),fill=(255,255,255))
    a=np.asarray(image.crop((x,y,x+w,y+h))).astype(np.int16)
    mask=(a.max(2)>=180)&(a.min(2)>=60)&(a.sum(2)>=420)
    scale=min(Fraction(120,w),Fraction(56,h));ow=max(1,round(w*scale));oh=max(1,round(h*scale))
    resized=Image.fromarray(mask.astype(np.uint8)).resize((ow,oh),Image.Resampling.NEAREST)
    canvas=Image.new('L',(128,64));canvas.paste(resized,((128-ow)//2,(64-oh)//2))
    image.save(dest/(name+'.png'));(dest/(name+'.mask')).write_bytes(canvas.tobytes())
    cases.append({'name':name,'kind':'prepared','bounds':list(bounds),'foreground_pixels':int(mask.sum())})
for name,color,fragment,kind in [('empty',(20,50,90),False,'empty'),
                                ('low-channel',(59,255,255),False,'empty'),
                                ('low-maximum',(179,179,179),False,'empty'),
                                ('fragment',(20,50,90),True,'unlocalized')]:
    image=Image.new('RGB',(128,64),color)
    if fragment:ImageDraw.Draw(image).rectangle((40,20,41,21),fill=(240,240,240))
    image.save(dest/(name+'.png'));cases.append({'name':name,'kind':kind,'bounds':None,'foreground_pixels':0})
(dest/'cases.json').write_text(json.dumps(cases,indent=2),encoding='utf-8')
print('Original action input fixtures:',len(cases))
