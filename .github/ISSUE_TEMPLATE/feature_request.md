---
name: Feature request
about: Suggest new feature (using user story)
title: ''
labels: ''
assignees: ''

---

## User story
1. As a {}
2. I want / need {}
3. So that {}

## Acceptance criteria
* Criterion 1
* Criterion 2
* ...

## Definition of done (DoD)
* All Acceptance criterias defined in the backlog item are met and verified
* Work done is pushed to the Github repository
* For each backlog item a branch is created
* Pull requests are created for each branch
* Pull requests are reviewed
* Corresponding branches are merged
* Software Bill of Materials in the planning document is updated if new dependencies were added
* The Wiki Section/Documentation corresponding to the work done is updated
* Unit tests are written for all new logic
* All unit tests must pass successfully in the CI pipeline
* Code passes cargo fmt (style) and cargo clippy (linting)
* All Developers assigned to a backlog item must communicate and coordinate with each other to complete the task

## DoD general criteria
* Feature has been fully implemented
* Feature has been merged into the mainline
* All acceptance criteria were met
* Product owner approved features
* All tests are passing
* Developers agreed to release
