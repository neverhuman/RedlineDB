# Jain Split Repo Standard

Source commit: `cc27936eb45006bda0cae85b0f578f4d5985991d`
Split repo: `jain`
Required check: `jain/required`

Required local commands are `just fast`, `just check`, `just score`, and
`just security`. Release-supporting repos also expose `just artifact-support`.

Score files under `.jankurai/` are produced by the pinned Jankurai lane.
Do not hand-edit them.
