# Papers behind AimAdapt's training decisions

Seven papers inform how AimAdapt rates a player, picks the difficulty of a run and advises on how long to train. Each has a note below: what it found, what AimAdapt takes from it and where it stops applying.

The summaries are written for this project. They are not a substitute for the papers.

| Paper | Used for | Full text here |
|---|---|---|
| [Wilson et al. 2019](#wilson-2019) | The 85% goal | [PDF](pdf/wilson-2019-eighty-five-percent-rule.pdf) |
| [Listman et al. 2021](#listman-2021) | How long to train in a day | [PDF](pdf/listman-2021-long-term-motor-learning.pdf) |
| [Czyż et al. 2024, transfer](#czyz-2024-transfer) | Mixing drills or playing them in blocks | [PDF](pdf/czyz-2024-contextual-interference-transfer.pdf) |
| [Czyż et al. 2024, retention](#czyz-2024-retention) | Mixing drills or playing them in blocks | [PDF](pdf/czyz-2024-contextual-interference-retention.pdf) |
| [Guadagnoli and Lee 2004](#guadagnoli-2004) | The mix of run kinds | Link only |
| [Watson and Pelli 1983](#watson-1983) | The rating model | Link only |
| [Kontsevich and Tyler 1999](#kontsevich-1999) | The rating model's slope | Link only |

## Copies and licences

The four PDFs in [pdf/](pdf) are unchanged copies of open-access articles published under the [Creative Commons Attribution 4.0 licence](https://creativecommons.org/licenses/by/4.0/) (CC BY 4.0). That licence allows sharing with credit to the authors, a link to the licence and a note of any changes. The credit is the citation in each note, and no changes were made. The copyright stays with the authors. AimAdapt's own licence does not cover these files.

The other three papers are not under an open licence, so only a citation and a link are kept here.

---

<a id="wilson-2019"></a>
## Wilson, Shenhav, Straccia and Cohen (2019): the 85% rule

Wilson RC, Shenhav A, Straccia M, Cohen JD. The Eighty Five Percent Rule for optimal learning. *Nature Communications* 10, 4646 (2019). [doi:10.1038/s41467-019-12552-4](https://doi.org/10.1038/s41467-019-12552-4). CC BY 4.0. [PDF](pdf/wilson-2019-eighty-five-percent-rule.pdf)

**What it found.** For a learner that improves by gradient descent on a two-choice task, there is a difficulty at which learning is fastest. With normally distributed noise that point is an error rate of about 15.87%, or roughly 85% correct. The authors derive the figure and then show it holds in simulations of a perceptron, a neural network trained on handwritten digits and a model of perceptual learning in monkeys.

**What AimAdapt takes from it.** The goal rate. A rating is the value a player holds at 85%, and runs at the player's level are planned for an 80 to 90% chance of success.

**Limits.**
- It is a result about learning algorithms. No human motor learning was measured.
- The exact figure depends on the shape of the noise. The paper gives about 82% correct for Laplacian noise and 75% for Cauchy noise.
- It is about two-choice classification. Hitting a target is a different kind of task, so 85% is a reasoned default here and has not been shown to be the best rate for aim.

<a id="listman-2021"></a>
## Listman, Tsay, Kim, Mackey and Heeger (2021): long-term learning in Aim Lab data

Listman JB, Tsay JS, Kim HE, Mackey WE, Heeger DJ. Long-Term Motor Learning in the "Wild" With High Volume Video Game Data. *Frontiers in Human Neuroscience* 15, 777779 (2021). [doi:10.3389/fnhum.2021.777779](https://doi.org/10.3389/fnhum.2021.777779). CC BY 4.0. [PDF](pdf/listman-2021-long-term-motor-learning.pdf)

**What it found.** The study used 682,564 sixty-second runs of one Aim Lab task (Gridshot) from 7,174 players, over up to 100 days.
- Improvement is steep at first and flattens with practice, as in laboratory motor learning.
- The gain carried to the next day grows with the number of runs played in a day and then turns down. Measured from the first run of the next day the peak is at about 75 runs, and 90% of that gain is reached by about 49. Measured from the second or third run of the next day the peak is lower (about 60 and 57 runs), because the first run of a day includes a warm-up cost.
- The authors' own summary: the greatest gains came with about an hour of practice in a day, 90% of the benefit came with about 30 minutes, and returns diminish after roughly 30 to 60 minutes.
- Between 40 and 60% of a day's gain is still there the next day.
- In their long-run model, a player starting at 2.78 hits a second reaches 3.17, 3.29, 3.35 and 3.39 after 10, 20, 30 and 40 days at ten runs a day, and 3.22, 3.35, 3.42 and 3.47 at twenty runs a day. Doubling the daily practice buys a small edge.
- Most players train far less than the peak. The median player played 7 runs a day.

**What AimAdapt takes from it.**
- The advice on session length. Half an hour a day is a sound default, and nothing here supports treating it as a ceiling: gains on a single task kept growing up to about an hour.
- Training on more days matters more than making one day long.
- A day's first run is a poor measure of what was learned the day before. AimAdapt's warm-up block and its caution about day-to-day changes follow from that.

**Limits.**
- One task. The players' time on other Aim Lab tasks and in games was not recorded, so "75 runs" is not their whole practice. A player who spends an hour on Gridshot has probably trained longer than that in total.
- Observational. Players who choose to play 75 runs a day differ from those who play 7, and no one was assigned a dose.
- The score is hits a second on static targets. Tracking and target switching may behave differently.

<a id="czyz-2024-transfer"></a>
## Czyż, Wójcik and Solarská (2024): mixed practice and transfer

Czyż SH, Wójcik AM, Solarská P. The effect of contextual interference on transfer in motor learning: a systematic review and meta-analysis. *Frontiers in Psychology* 15, 1377122 (2024). [doi:10.3389/fpsyg.2024.1377122](https://doi.org/10.3389/fpsyg.2024.1377122). CC BY 4.0. [PDF](pdf/czyz-2024-contextual-interference-transfer.pdf)

**What it found.** Contextual interference is the idea that practising several skills in a mixed order is harder in the moment and leaves more behind than practising each in its own block. This review covered 42 studies, 34 of them in the meta-analysis, and asked whether mixed practice helps a skill carry over to a new task.
- Overall there was a medium benefit for mixed order (standardized mean difference 0.55).
- In laboratory tasks the benefit was larger (0.75). In applied settings such as sport skills it was smaller (0.34) and not statistically significant.
- The effect was large in older adults (1.28), medium in adults (0.54) and close to nothing in young participants (0.12).

**What AimAdapt takes from it.** Read with the retention paper below, it is the reason Core Week keeps drills in blocks and does not shuffle them. The evidence for shuffling is strong in the laboratory and weak in applied training. The plan orders the kinds of run inside a block and leaves the blocks alone.

**Limits.** The applied studies are few and varied, so the result does not show that mixed practice fails in applied settings. No study in it is about aim training.

<a id="czyz-2024-retention"></a>
## Czyż, Wójcik, Solarská and Kiper (2024): mixed practice and retention

Czyż SH, Wójcik AM, Solarská P, Kiper P. High contextual interference improves retention in motor learning: systematic review and meta-analysis. *Scientific Reports* 14, 15974 (2024). [doi:10.1038/s41598-024-65753-3](https://doi.org/10.1038/s41598-024-65753-3). CC BY 4.0. [PDF](pdf/czyz-2024-contextual-interference-retention.pdf)

**What it found.** A companion to the paper above, asking whether mixed practice helps a skill last. The meta-analysis took in 54 studies.
- Overall there was a medium benefit for mixed order.
- The benefit came from laboratory studies. In applied settings it was close to nothing.
- By age the pattern matched the transfer paper: large in older adults, medium in adults, negligible in young participants.

**What AimAdapt takes from it.** The same decision as above: blocks stay.

**Limits.** As above. The adaptive review of November 2026 cited both of these papers as one "Czyż et al. 2024". The transfer figures (34 studies, 0.55) come from the first and the applied-setting retention result comes from this one.

<a id="guadagnoli-2004"></a>
## Guadagnoli and Lee (2004): the challenge point

Guadagnoli MA, Lee TD. Challenge Point: A Framework for Conceptualizing the Effects of Various Practice Conditions in Motor Learning. *Journal of Motor Behavior* 36(2), 212–224 (2004). [doi:10.3200/JMBR.36.2.212-224](https://doi.org/10.3200/JMBR.36.2.212-224). Not open access, so no copy is kept here.

**What it says.** Learning needs information, and how much a task gives depends on how hard it is for the person doing it. Too easy and there is nothing to learn from. Too hard and there is more than can be used. The best difficulty for learning sits above the best difficulty for performing well in the moment, and it rises with skill. It is a framework that organises earlier findings. It is not an experiment.

**What AimAdapt takes from it.** The mix of run kinds. Most runs sit at the player's level, some stretch runs are harder than is comfortable, and a few easy runs are below it. It also supports judging a stretch run against its own plan: a low hit rate on a hard run is what was asked for.

**Limits.** The framework gives no numbers. The shares in the Steady, Balanced and Push mixes are AimAdapt's own choices.

<a id="watson-1983"></a>
## Watson and Pelli (1983): QUEST

Watson AB, Pelli DG. QUEST: A Bayesian adaptive psychometric method. *Perception & Psychophysics* 33(2), 113–120 (1983). [doi:10.3758/BF03202828](https://doi.org/10.3758/BF03202828). Not under an open licence, so no copy is kept here.

**What it says.** To find a threshold (the stimulus level a person gets right a set share of the time), keep a probability distribution over where the threshold might be, test at its most likely value and update it after each answer. The method assumes the shape of the curve is known and only its position is unknown.

**What AimAdapt takes from it.** The rating is this kind of estimate: a distribution over the value a player holds at the goal rate, on a log scale, updated by every target. A calibration run places each target where the estimate currently sits.

**Limits.** QUEST assumes a threshold that stays put during the test and a slope known in advance. A player's skill moves and the slope differs between players and drills, which is why AimAdapt widens the estimate with time away and estimates the slope as well.

<a id="kontsevich-1999"></a>
## Kontsevich and Tyler (1999): estimating the slope too

Kontsevich LL, Tyler CW. Bayesian adaptive estimation of psychometric slope and threshold. *Vision Research* 39(16), 2729–2737 (1999). [doi:10.1016/S0042-6989(98)00285-5](https://doi.org/10.1016/S0042-6989(98)00285-5). Free to read at the publisher but not free to redistribute, so no copy is kept here.

**What it says.** The Psi method estimates the threshold and the slope together, choosing each trial to shrink the uncertainty about both. In their tests the threshold was found in about 30 trials. The slope took about 300.

**What AimAdapt takes from it.** The rating estimates both numbers on a grid, as Psi does. The tenfold gap between the two is the reason behind the plan's first adaptive change: one run of about 60 targets cannot settle the slope, so what was learned about it has to be carried from run to run.

**Limits.** Psi picks every trial for information. AimAdapt plays a whole run at one value after calibration, because changing difficulty target by target felt chaotic in play. That trades some efficiency for a run that feels like one drill.
