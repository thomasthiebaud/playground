# /learn

Help the user define a new learning project, then hand off to the learn skill to create it.

Ask the user these questions (all at once):

1. **What do you want to build?** (e.g. "a toy HTTP server", "a key-value store") — this becomes the project's final deliverable
2. **What's your starting point?** (languages, tools, frameworks you're already comfortable with)
3. **What's off-limits?** (libraries or abstractions you want to avoid to force learning)

Once you have clear answers to all three, use the Skill tool to invoke the `create-exercise` skill with the gathered spec.
