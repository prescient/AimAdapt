Name=AimAdapt Test - Drifting Pair
PlayerCharacters=Shooter
BotCharacters=Pair Rot.rot
IsChallenge=true
Timelimit=60.0
PlayerProfile=Shooter
AddedBots=Pair Rot.rot;Pair Rot.rot
PlayerMaxLives=0
BotMaxLives=0;0
PlayerTeam=1
BotTeams=2;2
ScorePerKill=0.0
ScorePerHit=0.1
ScoreMultAccuracy=true
MapName=drifting pair.json
MapScale=4.0
InvinciblePlayer=true
Timescale=1.0
IsTimeDilationActive=false
Description=Made up for AimAdapt's tests.[nl]Two drifting balls.
GameVersion=3.7.5

[Bot Rotation Profile]
Name=Pair Rot
ProfileNames=Drifter;Hopper
ProfileWeights=3.0;1.0

[Bot Profile]
Name=Drifter
DodgeProfileNames=Wander;Dart
DodgeProfileWeights=1.0;0.5
CharacterProfile=Drifter Ball
UseWeapons=false

[Bot Profile]
Name=Hopper
DodgeProfileNames=Wander
DodgeProfileWeights=1.0
CharacterProfile=Hop Ball
UseWeapons=false

[Character Profile]
Name=Shooter
MaxHealth=100.0
WeaponProfileNames=Steady Beam;;;;;;;
MaxSpeed=0.0
CameraOffset=X=0.000 Y=0.000 Z=0.000
// Note: color channels go from 0.0 to 1.0.
EnemyBodyColor=X=255.000 Y=0.000 Z=0.000
ThirdPersonCamera=false
AbilityProfileNames=;;;

[Character Profile]
Name=Drifter Ball
MaxHealth=100.0
MaxSpeed=600.0
Acceleration=2000.0
Friction=1.0
Gravity=0.0
MainBBType=Spheroid
MainBBHeight=100.0
MainBBRadius=50.0
MainBBHasHead=false
HealthRegenPerSec=5.0
AbilityProfileNames=Nudge.abilmelee;;;

[Character Profile]
Name=Hop Ball
MaxHealth=100.0
MaxSpeed=500.0
Gravity=1.0
JumpVelocity=600.0
MainBBType=Cylindrical
MainBBHeight=160.0
MainBBRadius=40.0
MainBBHasHead=false
AbilityProfileNames=Short Dash.abilmov;;;

[Dodge Profile]
Name=Wander
MaxTargetDistance=2400.0
MinTargetDistance=1800.0
ToggleLeftRight=true
MinLRTimeChange=0.4
MaxLRTimeChange=0.9
WaypointLogic=Ignore
LOSReactType=None

[Dodge Profile]
Name=Dart
MaxTargetDistance=2000.0
MinTargetDistance=1600.0
ToggleLeftRight=true
MinLRTimeChange=0.1
MaxLRTimeChange=0.3

[Melee Ability Profile]
Name=Nudge
HurtboxRadius=1500.0
FlatKnockbackHorizontal=300.0

[Movement Ability Profile]
Name=Short Dash
MainVelocity=2000.0
AbilityDuration=0.1

[Weapon Profile]
Name=Steady Beam
Type=Hitscan
ShotsPerClick=1
DamagePerShot=1.0
TimeBetweenShots=0.01
Category=FullyAuto
AAMode=0
HitscanRadius=0.01
MaxRecoilUp=0.0
CanAimDownSight=false

ADSResetsChargetrue
IsBurstWeapon=false

[Map Data]
{
    "version": "1.0.0",
    "materialSets": [
        {
            "wall": {
                "material": "Plain",
                "pack": "Default",
                "properties": [
                    {
                        "name": "Tint",
                        "value": "808080ff"
                    }
                ]
            }
        }
    ],
    "objects": [
        {
            "type": "brush",
            "mesh": "Cube",
            "name": "Default",
            "location": "0.000000, 0.000000, -16.000000",
            "rotation": "0.000000, 0.000000, 0.000000",
            "scale": "30.000000, 30.000000, 0.320000",
            "materialSets": [
                0
            ]
        },
        {
            "type": "brush",
            "mesh": "Cylinder",
            "name": "Default",
            "location": "900.000000, 0.000000, 200.000000",
            "rotation": "0.000000, 0.000000, 0.000000",
            "scale": "1.000000, 1.000000, 4.000000"
        },
        {
            "type": "brush",
            "mesh": "Cube",
            "name": "Default",
            "location": "-900.000000, 0.000000, 0.000000",
            "procedural": [
                {
                    "indices": [
                        0,
                        1,
                        2,
                        2,
                        3,
                        0
                    ],
                    "vertices": [
                        {
                            "location": "0.000000, 0.000000, 0.000000",
                            "normal": "1.0, 0.0, 0.0"
                        },
                        {
                            "location": "0.000000, 10.000000, 0.000000",
                            "normal": "1.0, 0.0, 0.0"
                        },
                        {
                            "location": "0.000000, 10.000000, 10.000000",
                            "normal": "1.0, 0.0, 0.0"
                        },
                        {
                            "location": "0.000000, 0.000000, 10.000000",
                            "normal": "1.0, 0.0, 0.0"
                        }
                    ]
                }
            ]
        },
        {
            "type": "gameObject",
            "name": "SpawnPoint",
            "location": "-600.000000, 0.000000, 90.000000",
            "rotation": "0.000000, 0.000000, 0.000000",
            "properties": [
                {
                    "name": "Name",
                    "value": ""
                },
                {
                    "name": "TeamMask",
                    "value": 1
                },
                {
                    "name": "Path",
                    "value": ""
                },
                {
                    "name": "LoopingPath",
                    "value": false
                },
                {
                    "name": "PermittedCharacterProfiles",
                    "value": ""
                },
                {
                    "name": "Weight",
                    "value": 1.0
                }
            ]
        },
        {
            "type": "gameObject",
            "name": "SpawnPoint",
            "location": "600.000000, -200.000000, 150.000000",
            "rotation": "0.000000, 0.000000, 180.000000",
            "properties": [
                {
                    "name": "TeamMask",
                    "value": 2
                },
                {
                    "name": "Weight",
                    "value": 2.0
                },
                {
                    "name": "PermittedCharacterProfiles",
                    "value": "Drifter Ball"
                }
            ]
        },
        {
            "type": "gameObject",
            "name": "SpawnVolume",
            "location": "600.000000, 200.000000, 150.000000",
            "rotation": "0.000000, 0.000000, 0.000000",
            "scale": "1.000000, 3.000000, 1.000000",
            "properties": [
                {
                    "name": "TeamMask",
                    "value": 2
                },
                {
                    "name": "Weight",
                    "value": 1.0
                }
            ]
        },
        {
            "type": "prop",
            "name": "Pebbles",
            "location": "0.0, 0.0, 0.0",
            "rotation": "0.0, 0.0, 0.0",
            "scale": "1.0, 1.0, 1.0",
            "properties": [
                {
                    "name": "EnableCollision",
                    "value": false
                }
            ]
        }
    ]
}
