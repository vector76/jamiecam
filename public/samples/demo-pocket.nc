; demo-pocket.nc
; Two-level stepped pocket: 100x100x20 mm stock, 10 mm flat endmill.
;
; @STOCK type=box width=100 depth=100 height=20 origin=0,0,0
; @TOOL number=1 type=flat_endmill diameter=10 flutes=4 material=carbide

G21        (metric)
G90        (absolute)

T1 M6      (tool change: 10 mm flat endmill)
S10000 M3  (spindle on CW)

; --- Outer shelf: Z=15, 5 mm deep ---
; Raster [10, 90] x [10, 90] at Z=15, stepover 8 mm

G0 Z25
G0 X10 Y10
G1 Z15 F500
G1 X90 F1000
G1 Y18
G1 X10
G1 Y26
G1 X90
G1 Y34
G1 X10
G1 Y42
G1 X90
G1 Y50
G1 X10
G1 Y58
G1 X90
G1 Y66
G1 X10
G1 Y74
G1 X90
G1 Y82
G1 X10
G1 Y90
G1 X90

; --- Inner pocket: Z=10, 10 mm deep ---
; Raster [30, 70] x [30, 70] at Z=10, stepover 8 mm

G0 Z25
G0 X30 Y30
G1 Z10 F500
G1 X70 F1000
G1 Y38
G1 X30
G1 Y46
G1 X70
G1 Y54
G1 X30
G1 Y62
G1 X70
G1 Y70
G1 X30

; --- Done ---

G0 Z25
M5
G0 X0 Y0
M30
