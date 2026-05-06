## Todo 2 (jolly is bad at lists)
🔎 = Feature/functionality missing that was present in ts forest

🆕 = New feature/functionality

**General**
* 🆕 Cooldowns should be cumulative. For example, the initial 10 second cooldown for !q is fine, but if someone quotes again within cooldown * 2 (20 seconds initially) the cooldown should then increase. I'm thinking just 1 extra second (making it 11 seconds until you can run it again, and 22 until the cooldown resets). This punishes over use and repeated use, since even a small cooldown doesn't seem to be enough to dissuade people to chill on the command spam. This concept should also be implemented for !lm, only waaay more aggressive. There should be a 300 second cooldown for last message on an individual user basis with the same "punishment" style increases. People use forest to bypass ignores and this is meant to dissuade that.
* 🔎 Self censorship
* 🔎 Whisper commands
* 🔎 Whisper that a command is disabled to the player who ran said command
* 🆕 Make faqs backfillable
* 🆕 Add cross server functionality to stats commands (this is mostly done, !lk, !ld, !vicitims, !fm, !lm, !ladv, and !top are missing, if this is intentional I can mark this one complete)
* 🆕 !delfaq aka !deletefaq, deletes the faq, freeing up the number. Should be done after faqs are backfillable. Should confirm in whisper.
* 🆕 !servers \<username> (not sure on the name), lists servers forest has data of the player on
* 🆕 !advancementcount \<advancement>, shows the number of times an advancement has been reached
* 🆕 !averageping, !ap, shows the average ping of the server as well as best and worst.
* 🔎 **bug**(?) don't record redundant advancements (from the queue or ever)
* 🔎 **bug**: fix discord bug where it fails to show /playtimegraph for a user without a join date
* 🔎 **bug**: fix discord bug(?) where blacklisted people's messages don't get sent to discord

**!quote**
* 🔎 Missing the basic 10 second cooldown from pre rewrite. (also needs some extra stuff noted in General)
* 🆕 Add support for !q \<username> \<keyword>
* 🆕 !q \<server>, without username specified, shows random quote from specified server


