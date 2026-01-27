#!/bin/bash

# sync_pr.sh
# Updates an existing PR by force-pushing updated changes to the sync branch
# Usage: ./sync_pr.sh <branch-name>

set -e  # Exit on error

# Load shared functions
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/sync_common.sh"

CURRENT_BRANCH=$(git branch --show-current)

echo -e "${BLUE}🔄 Updating existing PR${NC}"
echo ""

# Get branch name from argument
if [[ -n "$1" ]]; then
    BRANCH_NAME="$1"
else
    echo -e "${RED}❌ Error: Branch name required${NC}"
    echo -e "${YELLOW}Usage: $0 <branch-name>${NC}"
    echo -e "${YELLOW}Example: $0 add-feature-x${NC}"
    exit 1
fi

BRANCH_NAME=$(sanitize_branch_name "$BRANCH_NAME")

echo -e "${BLUE}Branch to update:${NC} ${BRANCH_NAME}"
echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

check_clean_state
update_codecrafters
push_to_remotes

# Check if sync branch exists locally, delete it if so
if git show-ref --verify --quiet "refs/heads/${BRANCH_NAME}"; then
    echo -e "${BLUE}🗑️  Deleting local branch: ${BRANCH_NAME}${NC}"
    git branch -D "$BRANCH_NAME"
    echo -e "${GREEN}✓${NC} Local branch deleted"
fi

# Create new sync branch (don't delete remote yet!)
echo ""
echo -e "${BLUE}🌿 Creating updated sync branch: ${BRANCH_NAME}${NC}"
git checkout -b "$BRANCH_NAME"

remove_cc_files
update_cargo_toml

# Check if there are any changes to commit
if [[ -n $(git status -s) ]]; then
    git commit -m "Clean up for portfolio sync

- Remove CodeCrafters-specific files
- Update Cargo.toml with proper project metadata"
    echo -e "${GREEN}✓${NC} Changes committed"
else
    echo -e "${YELLOW}ℹ️  No changes to commit${NC}"
fi

# Force push to update the existing PR
echo -e "${BLUE}📤 Force-pushing to update PR...${NC}"
git push origin "$BRANCH_NAME" --force

PR_URL=$(generate_pr_url "$BRANCH_NAME")

echo ""
echo -e "${GREEN}✅ PR updated successfully!${NC}"

display_pr_info "$BRANCH_NAME" "$PR_URL"
open_in_browser "$PR_URL"

# Return to original branch
echo -e "${BLUE}🔄 Returning to ${CURRENT_BRANCH} branch...${NC}"
git checkout "$CURRENT_BRANCH"

echo ""
echo -e "${GREEN}✨ Update complete!${NC}"
echo ""
echo -e "${BLUE}What happened:${NC}"
echo -e "  1. ${GREEN}✓${NC} Pushed latest changes to ${CODECRAFTERS_BRANCH}"
echo -e "  2. ${GREEN}✓${NC} Updated CodeCrafters remote"
echo -e "  3. ${GREEN}✓${NC} Recreated ${BRANCH_NAME} branch locally"
echo -e "  4. ${GREEN}✓${NC} Force-pushed to update the existing PR"
echo -e "  5. ${GREEN}✓${NC} Your PR remains open with updated changes"
echo ""
echo -e "${BLUE}To make more changes:${NC}"
echo -e "  1. Work on ${YELLOW}${CODECRAFTERS_BRANCH}${NC} branch"
echo -e "  2. Commit and push your changes"
echo -e "  3. Run: ${YELLOW}./sync_pr.sh ${BRANCH_NAME}${NC}"
echo ""
