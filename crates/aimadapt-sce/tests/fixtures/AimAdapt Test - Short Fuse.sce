Name=AimAdapt Test - Short Fuse
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]One still ball at a time carries a weapon ability whose weapon blows the ball up: two blasts of 20 kill its 30 health, so a ball the player doesn't shoot goes away by itself.
AimTypeTag=Clicking
Timelimit=45.0
PlayerProfile=Gunner
AddedBots=Fuse.bot
BotTeams=2
ScorePerKill=10.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Fuse Floor.json
MapScale=1.0
[Character Profile]
Name=Gunner
MaxHealth=1.0
MaxSpeed=0.0
WeaponProfileNames=Tapper;;;
[Weapon Profile]
Name=Tapper
Type=Hitscan
Category=SemiAuto
DamagePerShot=15.0
TimeBetweenShots=0.2
HitscanRadius=0.0
[Bot Profile]
Name=Fuse
CharacterProfile=Fuse Ball
UseWeapons=true
NoAiming=false
SeeThroughWalls=true
[Character Profile]
Name=Fuse Ball
MaxHealth=30.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=60.0
MainBBHasHead=false
EnemyBodyColor=X=0.900 Y=0.300 Z=0.200
MinRespawnDelay=1.5
MaxRespawnDelay=1.5
BlockSelfDamage=false
AbilityProfileNames=Short Fuse.abilwep;;;
[Weapon Ability Profile]
Name=Short Fuse
MaxCharges=1.0
ChargeTimer=0.05
ChargesRefundedOnKill=0.0
DelayAfterUse=0.0
FullyAuto=false
WeaponProfile=Slow Charge
BlockAttackTimer=0.0
AbilityBlockedWhenAttacking=false
AmmoPerShot=0
AIUseInCombat=true
AIUseOutOfCombat=true
AIUseOnGround=true
AIUseInAir=true
AIReuseTimer=0.02
AIMinSelfHealth=0.0
AIMaxSelfHealth=100.0
AIMinTargHealth=0.0
AIMaxTargHealth=100.0
AIMinTargDist=0.0
AIMaxTargDist=100000.0
AIMaxTargFOV=360.0
AIDamageReaction=false
[Weapon Profile]
Name=Slow Charge
Type=Projectile
ShotsPerClick=1
DamagePerShot=0.0
TimeBetweenShots=0.7
Category=SemiAuto
IsBurstWeapon=false
MuzzleVelocityMin=X=0.200 Y=0.000 Z=0.000
MuzzleVelocityMax=X=0.200 Y=0.000 Z=0.000
MaxTravelTime=0.5
GravityScale=0.0
MagazineMax=0
DelayBeforeShot=0.0
Explosive=true
Radius=5.0
DamageAtCenter=20.0
DamageAtEdge=20.0
SelfDamageMultiplier=1.0
ExplodesOnContactWithEnemy=false
ExplodesOnContactWithWorld=false
ExplodesOnNextAttack=false
DelayAfterSpawn=0.04
OriginOffset=X=0.000 Y=0.000 Z=0.000
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -1500, -250", "rotation": "0, 0, 0", "scale": "30, 30, 1"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1000, 0, -150", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
