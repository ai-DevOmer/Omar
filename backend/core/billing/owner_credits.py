"""
Owner Credits Module
Handles special credit allocation for the owner account.
"""
from decimal import Decimal
from typing import Tuple
from core.utils.logger import logger
from core.billing import repo as billing_repo
from core.billing.credits.manager import credit_manager

# Owner email
OWNER_EMAIL = "omerburp@gmail.com"
OWNER_CREDITS = Decimal("10000.00")  # 1 million credits (10000 dollars * 100 credits/dollar)

async def check_and_grant_owner_credits(account_id: str, email: str) -> Tuple[bool, str]:
    """
    Check if the user is the owner and grant them 1 million credits if needed.
    
    Args:
        account_id: User's account ID
        email: User's email address
        
    Returns:
        Tuple of (is_owner, message)
    """
    try:
        # Check if this is the owner
        if email.lower() != OWNER_EMAIL.lower():
            return False, "Not owner"
        
        logger.info(f"[OWNER CREDITS] Owner detected: {email}")
        
        # Get current balance
        account_data = await billing_repo.get_credit_account(account_id)
        if not account_data:
            logger.warning(f"[OWNER CREDITS] No account found for owner {account_id}")
            return False, "No account found"
        
        current_balance = Decimal(str(account_data.get('balance', 0)))
        
        # If balance is less than owner credits, top up to owner credits
        if current_balance < OWNER_CREDITS:
            amount_to_add = OWNER_CREDITS - current_balance
            
            result = await credit_manager.add_credits(
                account_id=account_id,
                amount=amount_to_add,
                is_expiring=False,
                description=f"Owner account top-up to {OWNER_CREDITS} credits",
                type='owner_grant'
            )
            
            if result.get('success'):
                logger.info(f"[OWNER CREDITS] ✅ Granted ${amount_to_add} to owner {email}. New balance: ${OWNER_CREDITS}")
                return True, f"Owner credits granted: ${amount_to_add}"
            else:
                logger.error(f"[OWNER CREDITS] Failed to grant credits to owner: {result.get('error')}")
                return False, f"Failed to grant credits: {result.get('error')}"
        else:
            logger.info(f"[OWNER CREDITS] Owner {email} already has sufficient credits: ${current_balance}")
            return True, f"Owner already has ${current_balance} credits"
            
    except Exception as e:
        logger.error(f"[OWNER CREDITS] Error checking/granting owner credits: {e}")
        return False, str(e)
