#!/usr/bin/env python3
import json,sys
x=json.load(open(sys.argv[1])) if len(sys.argv)>1 else {}
for a in x.get('advisories',{}).get('list',[]):
 print(a.get('id','unknown'), 'unknown')
print('advisory adjudication input accepted')
